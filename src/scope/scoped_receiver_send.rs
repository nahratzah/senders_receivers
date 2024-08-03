use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::scope::scope_fn_argument::ScopeFnArgument;
use crate::scope::scope_fn_send_receiver::ScopeFnSendReceiver;
use crate::traits::{Receiver, ReceiverOf};
use std::marker::PhantomData;

type NestedReceiver<Sch> = ScopeFnSendReceiver<Sch, Box<dyn Send + FnOnce(ScopeFnArgument<Sch>)>>;

/// A scoped receiver wrapper, that is sendable.
///
/// Signals are forwarded to the wrapped receiver.
/// The wrapped receiver lifetime is valid as long as this type exists.
pub struct ScopedReceiverSend<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    phantom: PhantomData<fn(Sch)>,
    nested: Option<NestedReceiver<Sch>>,
}

impl<Sch> ScopedReceiverSend<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    pub(super) fn new(nested: NestedReceiver<Sch>) -> Self {
        ScopedReceiverSend {
            phantom: PhantomData,
            nested: Some(nested),
        }
    }

    /// Invoke a callback on the nested receiver.
    /// If the callback panics, this function will ensure the scope-owning thread also panics.
    fn invoke<F>(mut self, f: F)
    where
        F: FnOnce(NestedReceiver<Sch>),
    {
        f(self.nested.take().unwrap());
    }
}

impl<Sch> Receiver for ScopedReceiverSend<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    fn set_done(self) {
        self.invoke(move |r| r.set_done())
    }

    fn set_error(self, error: Error) {
        self.invoke(move |r| r.set_error(error))
    }
}

impl<Sch> ReceiverOf<Sch, ()> for ScopedReceiverSend<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    fn set_value(self, scheduler: Sch, _: ()) {
        self.invoke(move |r| r.set_value(scheduler, ()))
    }
}
