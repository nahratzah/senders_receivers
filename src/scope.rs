//! Scope ensures that we can extend the lifetime of receivers in a safe way.
//!
//! The idea is taken from [std::thread::scope], although the implementation is not entirely the same.
//! - Different: we use [mem::transmute] to change lifetime.
//! - Same: we use [catch_unwind] to notice if the receiver panics, and if it does we'll propagate it to the caller.
use crate::scheduler::Scheduler;
use crate::traits::ReceiverOf;
use crate::tuple::Tuple;
use std::fmt;
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
    F: FnOnce(&ScopeImpl<ScopeDataPtr>) -> T,
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
    F: FnOnce(&ScopeImpl<ScopeDataSendPtr>) -> T,
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
    F: FnOnce(&ScopeImpl<ScopeDataSendPtr>) -> T,
{
    let (_, data) = ScopeDataSendPtr::new(move |_| {});
    let scope = ScopeImpl::new(data);
    f(&scope)
}

// Scope type

pub trait ScopeWrap<Sch, ReceiverType>: Clone + fmt::Debug
where
    Sch: Scheduler<LocalScheduler = Sch>,
    ReceiverType: ReceiverOf<Sch, ()>,
{
    /// Returned receiver type.
    ///
    /// This receiver will have static lifetime (enforced by this state).
    type WrapOutput: 'static + ReceiverOf<Sch, ()>;

    /// Wrap a function inside a scoped function.
    ///
    /// As long as the scoped-function remains in existence,
    /// the scope will remain alive.
    ///
    /// The returned receiver cannot [Send].
    fn wrap(&self, rcv: ReceiverType) -> Self::WrapOutput;
}

pub trait ScopeWrapSend<Sch, ReceiverType>:
    Clone + fmt::Debug + ScopeWrap<Sch, ReceiverType>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    ReceiverType: Send + ReceiverOf<Sch, ()>,
{
    /// Returned receiver type.
    ///
    /// This receiver will have static lifetime (enforced by this state) and implements [Send].
    type WrapSendOutput: 'static + ReceiverOf<Sch, ()> + Send;

    /// Wrap a function inside a scoped function.
    ///
    /// As long as the scoped-function remains in existence,
    /// the scope will remain alive.
    ///
    /// The returned receiver can [Send].
    fn wrap_send(&self, rcv: ReceiverType) -> Self::WrapSendOutput;
}

pub trait ScopeNest<Sch, Values, ReceiverType>: Clone + fmt::Debug
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    ReceiverType: ReceiverOf<Sch, Values>,
{
    type NewScopeData: ScopeData;
    type NewScopeType: Clone + fmt::Debug;
    type NewReceiver: ReceiverOf<Sch, Values>;

    fn new_scope(
        &self,
        rcv: ReceiverType,
    ) -> (
        Self::NewScopeType,
        Self::NewReceiver,
        ScopedRefMut<ReceiverType, Self::NewScopeData>,
    );
}

/// See [std::thread::Scope] for how this works.
#[derive(Clone)]
pub struct ScopeImpl<State>
where
    State: ScopeData,
{
    data: State,
}

impl<State> ScopeImpl<State>
where
    State: ScopeData,
{
    fn new(data: State) -> Self {
        Self { data }
    }
}

impl<Sch, ReceiverType, State> ScopeWrap<Sch, ReceiverType> for ScopeImpl<State>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    ReceiverType: ReceiverOf<Sch, ()>,
    State: ScopeData,
{
    type WrapOutput = ScopedReceiver<Sch>;

    fn wrap(&self, rcv: ReceiverType) -> Self::WrapOutput {
        let rcv_fn: Option<Box<dyn FnOnce(ScopeFnArgument<Sch>)>> = {
            let data = self.data.clone();
            Some(Box::new(move |scope_arg: ScopeFnArgument<Sch>| {
                data.run(move || {
                    match scope_arg {
                        ScopeFnArgument::Value(sch) => rcv.set_value(sch, ()),
                        ScopeFnArgument::Error(err) => rcv.set_error(err),
                        ScopeFnArgument::Done => rcv.set_done(),
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

impl<Sch, ReceiverType, State> ScopeWrapSend<Sch, ReceiverType> for ScopeImpl<State>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    ReceiverType: Send + ReceiverOf<Sch, ()>,
    State: Send + ScopeData,
{
    type WrapSendOutput = ScopedReceiverSend<Sch>;

    fn wrap_send(&self, rcv: ReceiverType) -> Self::WrapSendOutput {
        let rcv_fn: Option<Box<dyn Send + FnOnce(ScopeFnArgument<Sch>)>> = {
            let data = self.data.clone();
            Some(Box::new(move |scope_arg: ScopeFnArgument<Sch>| {
                data.run(move || {
                    match scope_arg {
                        ScopeFnArgument::Value(sch) => rcv.set_value(sch, ()),
                        ScopeFnArgument::Error(err) => rcv.set_error(err),
                        ScopeFnArgument::Done => rcv.set_done(),
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

impl<Sch, Values, ReceiverType, State> ScopeNest<Sch, Values, ReceiverType> for ScopeImpl<State>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    ReceiverType: ReceiverOf<Sch, Values>,
    State: ScopeData,
{
    type NewScopeData = State::NewScopeType<Sch, Values, ReceiverType>;
    type NewScopeType = ScopeImpl<Self::NewScopeData>;
    type NewReceiver = State::NewReceiver<Sch, Values, ReceiverType>;

    fn new_scope(
        &self,
        rcv: ReceiverType,
    ) -> (
        Self::NewScopeType,
        Self::NewReceiver,
        ScopedRefMut<ReceiverType, Self::NewScopeData>,
    ) {
        let (new_state, new_receiver, receiver_ref) = self.data.new_scope(rcv);
        (ScopeImpl::new(new_state), new_receiver, receiver_ref)
    }
}

impl<State> fmt::Debug for ScopeImpl<State>
where
    State: ScopeData,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}
