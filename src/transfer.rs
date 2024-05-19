use crate::errors::{Error, IsTuple};
use crate::scheduler::Scheduler;
use crate::then::Then;
use crate::traits::{BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender};
use core::ops::BitOr;
use std::marker::PhantomData;

/// Transfer to a different scheduler.
///
/// Further operations will run on the scheduler.
///
/// Example:
/// ```
/// use senders_receivers::{Just, Scheduler, Transfer, Then, sync_wait};
///
/// fn example(my_scheduler: impl Scheduler) {
///     let sender = Just::new((2, 3, 7))
///                  | Transfer::new(my_scheduler)
///                  | Then::new_fn(|(x, y, z)| {
///                      // This will run on `my_scheduler`.
///                      (x * y * z,)
///                  });
///     // And via sync_wait, we acquire the value on our own thread.
///     assert_eq!(
///         (42,),
///         sync_wait(sender).unwrap().unwrap());
/// }
/// ```
pub struct Transfer<Sch>
where
    Sch: Scheduler,
{
    target_scheduler: Sch,
}

impl<Sch> Transfer<Sch>
where
    Sch: Scheduler,
{
    pub fn new(target_scheduler: Sch) -> Transfer<Sch> {
        Transfer { target_scheduler }
    }
}

impl<Sch> Sender for Transfer<Sch> where Sch: Scheduler {}

impl<NestedSender, Sch> BindSender<NestedSender> for Transfer<Sch>
where
    Sch: Scheduler,
    NestedSender: TypedSender,
{
    type Output = TransferTS<NestedSender, Sch>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        TransferTS {
            nested,
            target_scheduler: self.target_scheduler,
        }
    }
}

pub struct TransferTS<NestedSender, Sch>
where
    NestedSender: TypedSender,
    Sch: Scheduler,
{
    nested: NestedSender,
    target_scheduler: Sch,
}

impl<NestedSender, Sch> TypedSender for TransferTS<NestedSender, Sch>
where
    NestedSender: TypedSender,
    Sch: Scheduler,
{
    type Value = NestedSender::Value;
    type Scheduler = Sch::LocalScheduler;

    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Self::Scheduler, Self::Value>,
    {
        let wrapped_receiver = ReceiverWrapper {
            nested: receiver,
            target_scheduler: self.target_scheduler,
            phantom: PhantomData,
        };

        self.nested.connect(wrapped_receiver)
    }
}

impl<NestedSender, Sch, BindSenderImpl> BitOr<BindSenderImpl> for TransferTS<NestedSender, Sch>
where
    BindSenderImpl: BindSender<TransferTS<NestedSender, Sch>>,
    NestedSender: TypedSender,
    Sch: Scheduler,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    Value: IsTuple,
{
    nested: NestedReceiver,
    target_scheduler: Sch,
    phantom: PhantomData<Value>,
}

impl<NestedReceiver, Sch, Value> Receiver for ReceiverWrapper<NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    Value: IsTuple,
{
    fn set_done(self) {
        self.nested.set_done()
    }
    fn set_error(self, error: Error) {
        self.nested.set_error(error)
    }
}

impl<PreviousScheduler, NestedReceiver, Sch, Value> ReceiverOf<PreviousScheduler, Value>
    for ReceiverWrapper<NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    PreviousScheduler: Scheduler,
    Value: IsTuple,
{
    fn set_value(self, _: PreviousScheduler, values: Value) {
        let sender = Then::new_fn(move |()| values).bind(self.target_scheduler.schedule());
        sender.connect(self.nested).start()
    }
}
