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
use scope_data::ScopeDataState;
pub(crate) use scope_data::{ScopeData, ScopeDataPtr, ScopeDataSendPtr};
use scope_fn_argument::ScopeFnArgument;
use scope_fn_receiver::ScopeFnReceiver;
use scope_fn_send_receiver::ScopeFnSendReceiver;
use scoped_receiver::ScopedReceiver;
use scoped_receiver_send::ScopedReceiverSend;

/// Create a [Scope], and run a senders-receivers chain within that scope.
pub(crate) fn scope<F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&ScopeImpl<'scope, ScopeDataPtr>) -> T,
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
pub(crate) fn scope_send<F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&ScopeImpl<'scope, ScopeDataSendPtr>) -> T,
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
    F: FnOnce(&ScopeImpl<'static, ScopeDataSendPtr>) -> T,
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

/// See [std::thread::Scope] for how this works.
#[derive(Clone)]
pub struct ScopeImpl<'scope, State>
where
    State: ScopeData,
{
    data: State,
    scope: PhantomData<&'scope mut &'scope ()>,
}

impl<'scope, State> ScopeImpl<'scope, State>
where
    State: ScopeData,
{
    fn new(data: State) -> Self {
        Self {
            data,
            scope: PhantomData,
        }
    }
}

impl<'scope, Sch, ReceiverType, State> ScopeWrap<Sch, ReceiverType> for ScopeImpl<'scope, State>
where
    Sch: Scheduler,
    ReceiverType: 'scope + ReceiverOf<Sch, ()>,
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

impl<'scope, Sch, ReceiverType, State> ScopeWrapSend<Sch, ReceiverType> for ScopeImpl<'scope, State>
where
    Sch: Scheduler,
    ReceiverType: 'scope + Send + ReceiverOf<Sch, ()>,
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

impl<'scope, Sch, Values, ReceiverType, State> ScopeNest<Sch, Values, ReceiverType>
    for ScopeImpl<'scope, State>
where
    Sch: Scheduler,
    Values: 'scope + Tuple,
    ReceiverType: 'scope + ReceiverOf<Sch, Values>,
    State: 'scope + ScopeData,
{
    type NewScopeData<'nested_scope> = State::NewScopeType<'nested_scope, 'scope, Sch, Values, ReceiverType>
    where Self: 'nested_scope, Values:'scope, ReceiverType: 'nested_scope;
    type NewScopeType<'nested_scope> = ScopeImpl<'nested_scope, Self::NewScopeData<'nested_scope>>
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

impl<'scope, State> fmt::Debug for ScopeImpl<'scope, State>
where
    State: ScopeData,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}
