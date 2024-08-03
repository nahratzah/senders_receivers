use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::scope::scope_fn_argument::ScopeFnArgument;
use crate::traits::{Receiver, ReceiverOf};
use std::marker::PhantomData;

/// Little marker type to ensure we remove the Sync+Send property from a type.
type PhantomUnsend = PhantomData<*const ()>;

/// Receiver that uses a function to deliver the signal.
///
/// This receiver is explicitly marked as `!Send` and `!Sync`.
pub struct ScopeFnReceiver<Sch, F>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    F: FnOnce(ScopeFnArgument<Sch>),
{
    phantom: PhantomData<fn(Sch)>,
    unsend: PhantomUnsend,
    f: F,
}

impl<Sch, F> ScopeFnReceiver<Sch, F>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    F: FnOnce(ScopeFnArgument<Sch>),
{
    pub(crate) fn new(f: F) -> Self {
        Self {
            phantom: PhantomData,
            unsend: PhantomData,
            f,
        }
    }
}

impl<Sch, F> Receiver for ScopeFnReceiver<Sch, F>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    F: FnOnce(ScopeFnArgument<Sch>),
{
    fn set_error(self, error: Error) {
        (self.f)(ScopeFnArgument::Error(error))
    }

    fn set_done(self) {
        (self.f)(ScopeFnArgument::Done)
    }
}

impl<Sch, F> ReceiverOf<Sch, ()> for ScopeFnReceiver<Sch, F>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    F: FnOnce(ScopeFnArgument<Sch>),
{
    fn set_value(self, sch: Sch, _: ()) {
        (self.f)(ScopeFnArgument::Value(sch))
    }
}
