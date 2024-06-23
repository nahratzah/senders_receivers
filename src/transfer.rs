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
pub struct Transfer<'a, Sch>
where
    Sch: Scheduler,
{
    phantom: PhantomData<&'a fn() -> Sch>,
    target_scheduler: Sch,
}

impl<'a, Sch> Transfer<'a, Sch>
where
    Sch: Scheduler,
{
    /// Create a new transfer.
    ///
    /// Note: we don't use from, because it reads really weird to write `Transfer::from(sch)`, when you mean "transfer to `sch`".
    pub fn new(target_scheduler: Sch) -> Self {
        Transfer {
            target_scheduler,
            phantom: PhantomData,
        }
    }
}

impl<'a, Sch> Sender for Transfer<'a, Sch> where Sch: Scheduler {}

impl<'a, NestedSender, Sch> BindSender<NestedSender> for Transfer<'a, Sch>
where
    Sch: Scheduler,
    NestedSender: TypedSender<'a>,
{
    type Output = TransferTS<'a, NestedSender, Sch>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        TransferTS {
            phantom: PhantomData,
            nested,
            target_scheduler: self.target_scheduler,
        }
    }
}

pub struct TransferTS<'a, NestedSender, Sch>
where
    NestedSender: TypedSender<'a>,
    Sch: Scheduler,
{
    phantom: PhantomData<&'a i32>,
    nested: NestedSender,
    target_scheduler: Sch,
}

impl<'a, NestedSender, Sch> TypedSender<'a> for TransferTS<'a, NestedSender, Sch>
where
    NestedSender: TypedSender<'a>,
    Sch: Scheduler,
{
    type Value = NestedSender::Value;
    type Scheduler = Sch::LocalScheduler;
}

impl<'a, ReceiverType, NestedSender, Sch> TypedSenderConnect<'a, ReceiverType>
    for TransferTS<'a, NestedSender, Sch>
where
    ReceiverType: ReceiverOf<Sch::LocalScheduler, NestedSender::Value>,
    NestedSender: 'a
        + TypedSender<'a>
        + TypedSenderConnect<
            'a,
            ReceiverWrapper<'a, ReceiverType, Sch, <NestedSender as TypedSender<'a>>::Value>,
        >,
    Sch: Scheduler,
    Sch::Sender: TypedSender<'a>
        + TypedSenderConnect<
            'a,
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

impl<'a, NestedSender, Sch, BindSenderImpl> BitOr<BindSenderImpl>
    for TransferTS<'a, NestedSender, Sch>
where
    BindSenderImpl: BindSender<TransferTS<'a, NestedSender, Sch>>,
    NestedSender: TypedSender<'a>,
    Sch: Scheduler,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<'a, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    Value: Tuple,
{
    nested: NestedReceiver,
    target_scheduler: Sch,
    phantom: PhantomData<&'a fn(Value) -> Value>,
}

impl<'a, NestedReceiver, Sch, Value> Receiver for ReceiverWrapper<'a, NestedReceiver, Sch, Value>
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

impl<'a, PreviousScheduler, NestedReceiver, Sch, Value> ReceiverOf<PreviousScheduler, Value>
    for ReceiverWrapper<'a, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    PreviousScheduler: Scheduler,
    Value: Tuple,
    Sch::Sender: TypedSenderConnect<
        'a,
        ContinuingReceiverWrapper<NestedReceiver, Sch::LocalScheduler, Value>,
    >,
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
