use crate::errors::{Error, Tuple};
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use std::marker::PhantomData;
use std::ops::BitOr;

/// Transfer to a different [Scheduler].
///
/// Subsequent [Sender] operations will run on the scheduler.
///
/// Example:
/// ```
/// use senders_receivers::{Just, Scheduler, Transfer, Then, sync_wait_send};
/// use threadpool::ThreadPool;
///
/// let pool = ThreadPool::with_name("example".into(), 1);
/// let sender = Just::from((2, 3, 7))
///              | Transfer::new(pool)
///              | Then::from(|(x, y, z)| {
///                  // This will run on `my_scheduler`.
///                  (x * y * z,)
///              });
/// // And via sync_wait_send, we acquire the value on our own thread.
/// // Note that we must use sync_wait_send, because ThreadPool will cross a thread boundary.
/// assert_eq!(
///     (42,),
///     sync_wait_send(sender).unwrap().unwrap());
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
    /// Create a new transfer.
    ///
    /// Note: we don't use from, because it reads really weird to write `Transfer::from(sch)`, when you mean "transfer to `sch`".
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
}

impl<ReceiverType, NestedSender, Sch> TypedSenderConnect<ReceiverType>
    for TransferTS<NestedSender, Sch>
where
    ReceiverType: ReceiverOf<Sch::LocalScheduler, NestedSender::Value>,
    NestedSender: TypedSender
        + TypedSenderConnect<ReceiverWrapper<ReceiverType, Sch, <NestedSender as TypedSender>::Value>>,
    Sch: Scheduler,
    Sch::Sender: TypedSenderConnect<
        ContinuingReceiverWrapper<ReceiverType, Sch::LocalScheduler, NestedSender::Value>,
    >,
{
    fn connect(self, nested: ReceiverType) -> impl OperationState {
        let receiver: ReceiverWrapper<ReceiverType, Sch, NestedSender::Value> = ReceiverWrapper {
            nested,
            target_scheduler: self.target_scheduler,
            phantom: PhantomData,
        };
        self.nested.connect(receiver)
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
    Value: Tuple,
{
    nested: NestedReceiver,
    target_scheduler: Sch,
    phantom: PhantomData<fn(Value) -> Value>,
}

impl<NestedReceiver, Sch, Value> Receiver for ReceiverWrapper<NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    Value: Tuple,
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
    Value: Tuple,
    Sch::Sender:
        TypedSenderConnect<ContinuingReceiverWrapper<NestedReceiver, Sch::LocalScheduler, Value>>,
{
    fn set_value(self, _: PreviousScheduler, values: Value) {
        self.target_scheduler
            .schedule()
            .connect(ContinuingReceiverWrapper {
                nested: self.nested,
                phantom: PhantomData,
                values,
            })
            .start();
    }
}

struct ContinuingReceiverWrapper<NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch, Value>,
    Sch: Scheduler,
    Value: Tuple,
{
    nested: NestedReceiver,
    phantom: PhantomData<Sch>,
    values: Value,
}

impl<NestedReceiver, Sch, Value> Receiver for ContinuingReceiverWrapper<NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch, Value>,
    Sch: Scheduler,
    Value: Tuple,
{
    fn set_done(self) {
        self.nested.set_done()
    }
    fn set_error(self, error: Error) {
        self.nested.set_error(error)
    }
}

impl<NestedReceiver, Sch, Value> ReceiverOf<Sch, ()>
    for ContinuingReceiverWrapper<NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch, Value>,
    Sch: Scheduler,
    Value: Tuple,
{
    fn set_value(self, sch: Sch, _: ()) {
        self.nested.set_value(sch, self.values)
    }
}
