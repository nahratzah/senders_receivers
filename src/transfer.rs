use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use crate::tuple::Tuple;
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
///                  // This will run on `pool`.
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
    NestedSender: TypedSender,
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
    NestedSender: TypedSender,
    Sch: Scheduler,
{
    phantom: PhantomData<&'a i32>,
    nested: NestedSender,
    target_scheduler: Sch,
}

impl<'a, NestedSender, Sch> TypedSender for TransferTS<'a, NestedSender, Sch>
where
    NestedSender: TypedSender,
    Sch: Scheduler,
{
    type Value = NestedSender::Value;
    type Scheduler = Sch::LocalScheduler;
}

impl<'a, ScopeImpl, ReceiverType, NestedSender, Sch> TypedSenderConnect<'a, ScopeImpl, ReceiverType>
    for TransferTS<'a, NestedSender, Sch>
where
    ReceiverType: ReceiverOf<Sch::LocalScheduler, NestedSender::Value>,
    NestedSender: 'a
        + TypedSender
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            ReceiverWrapper<'a, ScopeImpl, ReceiverType, Sch, <NestedSender as TypedSender>::Value>,
        >,
    Sch: Scheduler,
    Sch::Sender: TypedSender
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            ContinuingReceiverWrapper<ReceiverType, Sch::LocalScheduler, NestedSender::Value>,
        >,
    ScopeImpl: Clone,
{
    fn connect<'scope>(self, scope: &ScopeImpl, nested: ReceiverType) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        let receiver = ReceiverWrapper {
            nested,
            target_scheduler: self.target_scheduler,
            phantom: PhantomData,
            scope: scope.clone(),
        };
        self.nested.connect(scope, receiver)
    }
}

impl<'a, NestedSender, Sch, BindSenderImpl> BitOr<BindSenderImpl>
    for TransferTS<'a, NestedSender, Sch>
where
    BindSenderImpl: BindSender<TransferTS<'a, NestedSender, Sch>>,
    NestedSender: TypedSender,
    Sch: Scheduler,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<'a, ScopeImpl, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    Value: 'a + Tuple,
{
    nested: NestedReceiver,
    target_scheduler: Sch,
    phantom: PhantomData<&'a fn(Value) -> Value>,
    scope: ScopeImpl,
}

impl<'a, ScopeImpl, NestedReceiver, Sch, Value> Receiver
    for ReceiverWrapper<'a, ScopeImpl, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    Value: 'a + Tuple,
{
    fn set_done(self) {
        self.nested.set_done()
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error)
    }
}

impl<'a, ScopeImpl, PreviousScheduler, NestedReceiver, Sch, Value>
    ReceiverOf<PreviousScheduler, Value>
    for ReceiverWrapper<'a, ScopeImpl, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Value>,
    Sch: Scheduler,
    PreviousScheduler: Scheduler,
    Value: Tuple,
    Sch::Sender: TypedSenderConnect<
        'a,
        ScopeImpl,
        ContinuingReceiverWrapper<NestedReceiver, Sch::LocalScheduler, Value>,
    >,
{
    fn set_value(self, _: PreviousScheduler, values: Value) {
        self.target_scheduler
            .schedule()
            .connect(
                &self.scope,
                ContinuingReceiverWrapper {
                    nested: self.nested,
                    phantom: PhantomData,
                    values,
                },
            )
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
