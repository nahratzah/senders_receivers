//! Scope ensures that we can extend the lifetime of receivers in a safe way.
//!
//! The idea is taken from [std::thread::scope], although the implementation is not entirely the same.
//! - Different: we use [mem::transmute] to change lifetime.
//! - Same: we use [catch_unwind] to notice if the receiver panics, and if it does we'll propagate it to the caller.
use crate::scheduler::Scheduler;
use crate::traits::ReceiverOf;
use crate::tuple::Tuple;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::thread;

mod receiver;
pub(crate) mod scope_data;
pub(crate) mod scope_fn_argument;
pub(crate) mod scope_fn_receiver;
pub(crate) mod scope_fn_send_receiver;
pub(crate) mod scoped_receiver;
pub(crate) mod scoped_receiver_send;

use crate::refs::ScopedRefMut;
use receiver::{InnerScopeReceiver, InnerScopeSendReceiver};
use scope_data::ScopeDataState;
pub(crate) use scope_data::{ScopeData, ScopeDataPtr, ScopeDataSendPtr};
use scope_fn_argument::ScopeFnArgument;
use scope_fn_receiver::ScopeFnReceiver;
use scope_fn_send_receiver::ScopeFnSendReceiver;
use scoped_receiver::ScopedReceiver;
use scoped_receiver_send::ScopedReceiverSend;

/// Create a [Scope], and run a senders-receivers chain within that scope.
pub(crate) fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&ScopeImpl<'scope, 'env, ScopeDataPtr>) -> T,
{
    let main_thread = thread::current();
    let (state, data) = ScopeDataPtr::new(move |_| {
        main_thread.unpark();
    });

    let result = {
        let scope = ScopeImpl::new(data);
        catch_unwind(AssertUnwindSafe(|| f(&scope)))
    };

    // Wait until all threads are finished.
    while state.running() {
        thread::park();
    }

    match result {
        Err(e) => resume_unwind(e),
        Ok(_) if state.a_thread_panicked() => {
            panic!("a scoped thread panicked")
        }
        Ok(result) => result,
    }
}

/// Create a [Scope], and run a senders-receivers chain within that scope.
pub(crate) fn scope_send<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&ScopeImpl<'scope, 'env, ScopeDataSendPtr>) -> T,
{
    let main_thread = thread::current();
    let (state, data) = ScopeDataSendPtr::new(move |_| {
        main_thread.unpark();
    });

    let result = {
        let scope = ScopeImpl::new(data);
        catch_unwind(AssertUnwindSafe(|| f(&scope)))
    };

    // Wait until all threads are finished.
    while state.running() {
        thread::park();
    }

    match result {
        Err(e) => resume_unwind(e),
        Ok(_) if state.a_thread_panicked() => {
            panic!("a scoped thread panicked")
        }
        Ok(result) => result,
    }
}

/// Create a scope for [start_detached](crate::start_detached::start_detached).
/// This scope doesn't keep anything alive.
/// And to maintain correctness, therefore requires that all wrapped references don't have lifetime constraints.
pub(crate) fn detached_scope<F, T>(f: F) -> T
where
    F: FnOnce(&ScopeImpl<'static, 'static, ScopeDataSendPtr>) -> T,
{
    let (_, data) = ScopeDataSendPtr::new(move |_| {});
    let scope = ScopeImpl::new(data);
    return f(&scope);
}

// Scope type

pub trait ScopeWrap<Sch, ReceiverType>: Clone + fmt::Debug
where
    Sch: Scheduler,
    ReceiverType: ReceiverOf<Sch, ()>,
{
    /// Wrap a function inside a scoped function.
    ///
    /// As long as the scoped-function remains in existence,
    /// the scope will remain alive.
    ///
    /// The returned receiver cannot [Send].
    fn wrap(&self, rcv: ReceiverType) -> impl 'static + ReceiverOf<Sch, ()>;
}

pub trait ScopeWrapSend<Sch, ReceiverType>:
    Clone + fmt::Debug + ScopeWrap<Sch, ReceiverType>
where
    Sch: Scheduler,
    ReceiverType: Send + ReceiverOf<Sch, ()>,
{
    /// Wrap a function inside a scoped function.
    ///
    /// As long as the scoped-function remains in existence,
    /// the scope will remain alive.
    ///
    /// The returned receiver can [Send].
    fn wrap_send(&self, rcv: ReceiverType) -> impl 'static + ReceiverOf<Sch, ()> + Send;
}

pub trait ScopeNest<Sch, Values, ReceiverType>: Clone + fmt::Debug
where
    Sch: Scheduler,
    Values: Tuple,
    ReceiverType: ReceiverOf<Sch, Values>,
{
    type NewScopeData<'nested_scope>: 'nested_scope + ScopeData
    where
        Self: 'nested_scope,
        ReceiverType: 'nested_scope;
    type NewScopeType<'nested_scope>: 'nested_scope + Clone + fmt::Debug
    where
        Self: 'nested_scope,
        ReceiverType: 'nested_scope;
    type NewReceiver<'nested_scope>: 'nested_scope + ReceiverOf<Sch, Values>
    where
        Self: 'nested_scope,
        ReceiverType: 'nested_scope;

    fn new_scope<'nested_scope>(
        &self,
        rcv: ReceiverType,
    ) -> (
        Self::NewScopeType<'nested_scope>,
        Self::NewReceiver<'nested_scope>,
        ScopedRefMut<'nested_scope, ReceiverType, Self::NewScopeData<'nested_scope>>,
    )
    where
        Self: 'nested_scope,
        ReceiverType: 'nested_scope;
}

//#[deprecated(note="this is getting split into [ScopeWrap] and [ScopeNest]")]
pub trait Scope<'scope, 'env>: Clone + fmt::Debug
where
    'env: 'scope,
{
    type NewScopeType<'nested_scope, Sch, Values, Rcv>: Scope<'nested_scope, 'scope>
    where
        'scope: 'nested_scope,
        Sch: Scheduler,
        Values: Tuple,
        Rcv: 'scope + ReceiverOf<Sch, Values>;
    type NewScopeReceiver<Sch, Values, Rcv>: ReceiverOf<Sch, Values>
    where
        Sch: Scheduler,
        Values: Tuple,
        Rcv: 'scope + ReceiverOf<Sch, Values>;

    /// Wrap a function inside a scoped function.
    ///
    /// As long as the scoped-function remains in existence,
    /// the scope will remain alive.
    ///
    /// The returned receiver cannot [Send].
    fn wrap<ReceiverType, Sch>(&self, rcv: ReceiverType) -> impl 'static + ReceiverOf<Sch, ()>
    where
        ReceiverType: 'scope + ReceiverOf<Sch, ()>,
        Sch: Scheduler;

    /// Given a receiver, attach a scope to it, so you can have a nested scope.
    fn new_scope<'nested_scope, Sch, Value, Rcv>(
        &self,
        rcv: Rcv,
    ) -> (
        Self::NewScopeType<'nested_scope, Sch, Value, Rcv>,
        Self::NewScopeReceiver<Sch, Value, Rcv>,
        ScopedRefMut<'nested_scope, Rcv, Self::NewScopeType<'nested_scope, Sch, Value, Rcv>>,
    )
    where
        'scope: 'nested_scope,
        Sch: Scheduler,
        Value: Tuple,
        Rcv: 'scope + ReceiverOf<Sch, Value>;
}

//#[deprecated(note="this is getting split into [ScopeWrapSend] and [ScopeNest]")]
pub trait ScopeSend<'scope, 'env>: Scope<'scope, 'env> + Send + Sync
where
    'env: 'scope,
{
    // If Sch, Values, and Rcv are all Send, then this type will be Send+Sync.
    type NewScopeSendType<'nested_scope, Sch, Values, Rcv>: Scope<'nested_scope, 'scope>
    where
        'scope: 'nested_scope,
        Sch: Scheduler,
        Values: Tuple,
        Rcv: 'scope + ReceiverOf<Sch, Values>;
    type NewScopeSendReceiver<Sch, Values, Rcv>: ReceiverOf<Sch, Values>
    where
        Sch: Scheduler,
        Values: Tuple,
        Rcv: 'scope + ReceiverOf<Sch, Values>;

    /// Wrap a function inside a scoped function.
    ///
    /// As long as the scoped-function remains in existence,
    /// the scope will remain alive.
    ///
    /// The returned receiver can [Send].
    fn wrap_send<ReceiverType, Sch>(
        &self,
        rcv: ReceiverType,
    ) -> impl 'static + ReceiverOf<Sch, ()> + Send
    where
        ReceiverType: 'scope + ReceiverOf<Sch, ()> + Send,
        Sch: Scheduler;

    /// Given a receiver, attach a scope to it, so you can have a nested scope.
    fn new_scope_send<'nested_scope, Sch, Value, Rcv>(
        &self,
        rcv: Rcv,
    ) -> (
        Self::NewScopeSendType<'nested_scope, Sch, Value, Rcv>,
        Self::NewScopeSendReceiver<Sch, Value, Rcv>,
    )
    where
        'scope: 'nested_scope,
        Sch: Send + Scheduler,
        Value: Send + Tuple,
        Rcv: 'scope + Send + ReceiverOf<Sch, Value>;
}

/// See [std::thread::Scope] for how this works.
#[derive(Clone)]
pub struct ScopeImpl<'scope, 'env, State>
where
    'env: 'scope,
    State: ScopeData,
{
    data: State,
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

impl<'scope, 'env, State> ScopeImpl<'scope, 'env, State>
where
    'env: 'scope,
    State: ScopeData,
{
    fn new(data: State) -> Self {
        Self {
            data,
            scope: PhantomData,
            env: PhantomData,
        }
    }
}

impl<'scope, 'env, Sch, ReceiverType, State> ScopeWrap<Sch, ReceiverType>
    for ScopeImpl<'scope, 'env, State>
where
    'env: 'scope,
    Sch: Scheduler,
    ReceiverType: 'env + ReceiverOf<Sch, ()>,
    State: ScopeData,
{
    fn wrap(&self, rcv: ReceiverType) -> impl 'static + ReceiverOf<Sch, ()> {
        let rcv_fn: Option<Box<dyn FnOnce(ScopeFnArgument<Sch>)>> = {
            let data = self.data.clone();
            Some(Box::new(move |scope_arg: ScopeFnArgument<Sch>| {
                data.run(move || {
                    match scope_arg {
                        ScopeFnArgument::ValueSignal(sch) => rcv.set_value(sch, ()),
                        ScopeFnArgument::ErrorSignal(err) => rcv.set_error(err),
                        ScopeFnArgument::DoneSignal => rcv.set_done(),
                    };
                });
            }))
        };

        // Remove the lifetime constraint.
        let rcv_fn = unsafe {
            mem::transmute::<
                Option<Box<dyn FnOnce(ScopeFnArgument<Sch>)>>,
                Option<Box<dyn FnOnce(ScopeFnArgument<Sch>)>>,
            >(rcv_fn)
            .take()
            .unwrap()
        };

        ScopedReceiver::new(ScopeFnReceiver::new(rcv_fn))
    }
}

impl<'scope, 'env, Sch, ReceiverType, State> ScopeWrapSend<Sch, ReceiverType>
    for ScopeImpl<'scope, 'env, State>
where
    'env: 'scope,
    Sch: Scheduler,
    ReceiverType: 'env + Send + ReceiverOf<Sch, ()>,
    State: Send + ScopeData,
{
    fn wrap_send(&self, rcv: ReceiverType) -> impl 'static + Send + ReceiverOf<Sch, ()> {
        let rcv_fn: Option<Box<dyn Send + FnOnce(ScopeFnArgument<Sch>)>> = {
            let data = self.data.clone();
            Some(Box::new(move |scope_arg: ScopeFnArgument<Sch>| {
                data.run(move || {
                    match scope_arg {
                        ScopeFnArgument::ValueSignal(sch) => rcv.set_value(sch, ()),
                        ScopeFnArgument::ErrorSignal(err) => rcv.set_error(err),
                        ScopeFnArgument::DoneSignal => rcv.set_done(),
                    };
                });
            }))
        };

        // Remove the lifetime constraint.
        let rcv_fn = unsafe {
            mem::transmute::<
                Option<Box<dyn Send + FnOnce(ScopeFnArgument<Sch>)>>,
                Option<Box<dyn Send + FnOnce(ScopeFnArgument<Sch>)>>,
            >(rcv_fn)
            .take()
            .unwrap()
        };

        ScopedReceiverSend::new(ScopeFnSendReceiver::new(rcv_fn))
    }
}

impl<'scope, 'env, Sch, Values, ReceiverType, State> ScopeNest<Sch, Values, ReceiverType>
    for ScopeImpl<'scope, 'env, State>
where
    'env: 'scope,
    Sch: Scheduler,
    Values: 'scope + Tuple,
    ReceiverType: 'scope + ReceiverOf<Sch, Values>,
    State: 'scope + ScopeData,
{
    type NewScopeData<'nested_scope> = State::NewScopeType<'nested_scope, 'scope, Sch, Values, ReceiverType>
    where Self: 'nested_scope, Values:'scope, ReceiverType: 'nested_scope;
    type NewScopeType<'nested_scope> = ScopeImpl<'nested_scope, 'scope, Self::NewScopeData<'nested_scope>>
    where Self: 'nested_scope, Values:'scope, ReceiverType: 'nested_scope;
    type NewReceiver<'nested_scope> = State::NewReceiver<'nested_scope, 'scope, Sch, Values, ReceiverType>
    where Self: 'nested_scope, Values:'scope, ReceiverType: 'nested_scope;

    fn new_scope<'nested_scope>(
        &self,
        rcv: ReceiverType,
    ) -> (
        Self::NewScopeType<'nested_scope>,
        Self::NewReceiver<'nested_scope>,
        ScopedRefMut<'nested_scope, ReceiverType, Self::NewScopeData<'nested_scope>>,
    )
    where
        Self: 'nested_scope,
        ReceiverType: 'nested_scope,
    {
        let (new_state, new_receiver, receiver_ref) = self.data.new_scope(rcv);
        (ScopeImpl::new(new_state), new_receiver, receiver_ref)
    }
}

impl<'scope, 'env, State> fmt::Debug for ScopeImpl<'scope, 'env, State>
where
    'env: 'scope,
    State: ScopeData,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}
