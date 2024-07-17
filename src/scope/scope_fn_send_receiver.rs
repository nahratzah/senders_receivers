use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::scope::scope_fn_argument::ScopeFnArgument;
use crate::traits::{Receiver, ReceiverOf};
use std::marker::PhantomData;

/// Receiver that uses a function to deliver the signal.
///
/// This receiver allows for `Send`.
pub struct ScopeFnSendReceiver<Sch, F>
where
    Sch: Scheduler,
    F: FnOnce(ScopeFnArgument<Sch>) + Send,
{
    phantom: PhantomData<fn(Sch)>,
    f: F,
}

impl<Sch, F> ScopeFnSendReceiver<Sch, F>
where
    Sch: Scheduler,
    F: FnOnce(ScopeFnArgument<Sch>) + Send,
{
    pub(crate) fn new(f: F) -> Self {
        Self {
            phantom: PhantomData,
            f,
        }
    }
}

impl<Sch, F> Receiver for ScopeFnSendReceiver<Sch, F>
where
    Sch: Scheduler,
    F: FnOnce(ScopeFnArgument<Sch>) + Send,
{
    fn set_error(self, error: Error) {
        (self.f)(ScopeFnArgument::Error(error))
    }

    fn set_done(self) {
        (self.f)(ScopeFnArgument::Done)
    }
}

impl<Sch, F> ReceiverOf<Sch, ()> for ScopeFnSendReceiver<Sch, F>
where
    Sch: Scheduler,
    F: FnOnce(ScopeFnArgument<Sch>) + Send,
{
    fn set_value(self, sch: Sch, _: ()) {
        (self.f)(ScopeFnArgument::Value(sch))
    }
}
