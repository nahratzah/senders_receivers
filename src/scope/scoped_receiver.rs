use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::scope::scope_data::ScopeData;
use crate::scope::scope_fn_argument::ScopeFnArgument;
use crate::scope::scope_fn_receiver::ScopeFnReceiver;
use crate::traits::{Receiver, ReceiverOf};
use std::fmt;
use std::marker::PhantomData;

type NestedReceiver<Sch> = ScopeFnReceiver<Sch, Box<dyn FnOnce(ScopeFnArgument<Sch>)>>;

/// A scoped receiver wrapper.
///
/// Signals are forwarded to the wrapped receiver.
/// The wrapped receiver lifetime is valid as long as this type exists.
pub struct ScopedReceiver<Sch, StatePtr>
where
    Sch: Scheduler,
    StatePtr: ScopeData,
{
    phantom: PhantomData<fn(Sch)>,
    data: StatePtr,
    nested: Option<NestedReceiver<Sch>>,
}

impl<Sch, StatePtr> ScopedReceiver<Sch, StatePtr>
where
    Sch: Scheduler,
    StatePtr: ScopeData,
{
    pub(super) fn new(data: StatePtr, nested: NestedReceiver<Sch>) -> Self {
        ScopedReceiver {
            phantom: PhantomData,
            data,
            nested: Some(nested),
        }
    }

    /// Invoke a callback on the nested receiver.
    /// If the callback panics, this function will ensure the scope-owning thread also panics.
    fn invoke<F>(mut self, f: F)
    where
        F: FnOnce(NestedReceiver<Sch>),
    {
        let nested_ref = &mut self.nested;
        self.data.run(move || f(nested_ref.take().unwrap()));
    }
}

impl<Sch, StatePtr> Receiver for ScopedReceiver<Sch, StatePtr>
where
    Sch: Scheduler,
    StatePtr: ScopeData,
{
    fn set_done(self) {
        self.invoke(move |r| r.set_done())
    }

    fn set_error(self, error: Error) {
        self.invoke(move |r| r.set_error(error))
    }
}

impl<Sch, StatePtr> ReceiverOf<Sch, ()> for ScopedReceiver<Sch, StatePtr>
where
    Sch: Scheduler,
    StatePtr: ScopeData,
{
    fn set_value(self, scheduler: Sch, _: ()) {
        self.invoke(move |r| r.set_value(scheduler, ()))
    }
}

impl<Sch, StatePtr> fmt::Debug for ScopedReceiver<Sch, StatePtr>
where
    Sch: Scheduler,
    StatePtr: ScopeData,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}
