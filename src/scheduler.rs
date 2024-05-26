use crate::errors::{Error, IsTuple};
use crate::just::Just;
use crate::just_done::JustDone;
use crate::just_error::JustError;
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use std::ops::BitOr;
use threadpool::ThreadPool;

/// Schedulers are things that can do work.
/// All sender chains run on a scheduler.
///
/// Most of the time, you'll use a scheduler in combination with [Transfer](crate::Transfer).
/// But a scheduler can be used directly (which is what [Transfer](crate::Transfer) does under the hood).
/// Unfortunately, I can't make the `|` operator work for [Scheduler::Sender], so we must use the [`BindSender::bind`] function instead.
/// ```
/// use senders_receivers::{BindSender, Scheduler, Then, sync_wait_send};
/// use threadpool::ThreadPool;
///
/// let pool = ThreadPool::with_name("example".into(), 1);
/// let sender = pool.schedule()
///              | Then::from(|()| {
///                  // This computation will run on the scheduler.
///                  let mut s = 0_i32;
///                  for n in 1..101 {
///                      s += n;
///                  }
///                  (s,)
///              });
/// assert_eq!(
///     5050,
///     sync_wait_send(sender).unwrap().unwrap().0);
/// ```
pub trait Scheduler: Eq + Clone {
    /// Mark if the scheduler may block the caller, when started.
    /// If this is `false`, you are guaranteed that the sender will complete independently from the operation-state start method.
    /// But if it returns `true`, the scheduler will complete before returning from the operation-state start method.
    const EXECUTION_BLOCKS_CALLER: bool;
    /// The scheduler that's passed on along the value signal.
    /// Most of the time, this would be the same as this.
    ///
    /// But if for example an embarrisingly-parallel scheduler is used,
    /// the LocalScheduler would represent the scheduler bound to the thread that was selected.
    type LocalScheduler: Scheduler<Sender = Self::Sender>;

    /// The [TypedSender] returned by the scheduler.
    type Sender: TypedSender<Scheduler = Self::LocalScheduler, Value = ()>;

    /// Create a [Self::Sender] that'll run on this scheduler.
    fn schedule(&self) -> Self::Sender;

    /// Create a sender that'll run on this scheduler, that produces a value signal.
    fn schedule_value<Tuple: IsTuple>(&self, values: Tuple) -> Just<Self, Tuple> {
        Just::with_scheduler(self.clone(), values)
    }

    /// Create a sender associated with this scheduler, that produces an error signal.
    fn schedule_error<Tuple: IsTuple>(&self, error: Error) -> JustError<Self, Tuple> {
        JustError::<Self, Tuple>::new(error)
    }

    /// Create a sender associated with this scheduler, that produces a done signal.
    fn schedule_done<Tuple: IsTuple>(&self) -> JustDone<Self, Tuple> {
        JustDone::<Self, Tuple>::new()
    }
}

/// An immediate-scheduler is a [Scheduler] which runs any tasks on it immediately.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct ImmediateScheduler {}

/// This scheduler is a basic scheduler, that just runs everything immediately.
impl Scheduler for ImmediateScheduler {
    const EXECUTION_BLOCKS_CALLER: bool = true;
    type LocalScheduler = ImmediateScheduler;
    type Sender = ImmediateSender;

    fn schedule(&self) -> Self::Sender {
        ImmediateSender {}
    }
}

pub struct ImmediateSender {}

impl TypedSender for ImmediateSender {
    type Scheduler = ImmediateScheduler;
    type Value = ();
}

impl<ReceiverType> TypedSenderConnect<ReceiverType> for ImmediateSender
where
    ReceiverType: ReceiverOf<
        <ImmediateSender as TypedSender>::Scheduler,
        <ImmediateSender as TypedSender>::Value,
    >,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        ImmediateOperationState { receiver }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for ImmediateSender
where
    BindSenderImpl: BindSender<ImmediateSender>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ImmediateOperationState<ReceiverType>
where
    ReceiverType: ReceiverOf<ImmediateScheduler, ()>,
{
    receiver: ReceiverType,
}

impl<ReceiverType> OperationState for ImmediateOperationState<ReceiverType>
where
    ReceiverType: ReceiverOf<ImmediateScheduler, ()>,
{
    fn start(self) {
        self.receiver.set_value(ImmediateScheduler {}, ());
    }
}

impl Scheduler for ThreadPool {
    const EXECUTION_BLOCKS_CALLER: bool = false;
    type LocalScheduler = ThreadPool;
    type Sender = ThreadPoolSender;

    fn schedule(&self) -> Self::Sender {
        ThreadPoolSender { pool: self.clone() }
    }
}

pub struct ThreadPoolSender {
    pool: ThreadPool,
}

impl TypedSender for ThreadPoolSender {
    type Value = ();
    type Scheduler = ThreadPool;
}

impl<ReceiverType> TypedSenderConnect<ReceiverType> for ThreadPoolSender
where
    ReceiverType: Send
        + 'static
        + ReceiverOf<
            <ThreadPoolSender as TypedSender>::Scheduler,
            <ThreadPoolSender as TypedSender>::Value,
        >,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        ThreadPoolOperationState {
            pool: self.pool,
            receiver,
        }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for ThreadPoolSender
where
    BindSenderImpl: BindSender<ThreadPoolSender>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ThreadPoolOperationState<Receiver>
where
    Receiver: ReceiverOf<ThreadPool, ()> + Send + 'static,
{
    pool: ThreadPool,
    receiver: Receiver,
}

impl<Receiver> OperationState for ThreadPoolOperationState<Receiver>
where
    Receiver: ReceiverOf<ThreadPool, ()> + Send + 'static,
{
    fn start(self) {
        let pool = self.pool.clone();
        let receiver = self.receiver;
        self.pool.execute(move || receiver.set_value(pool, ()));
    }
}

/// This trait allows us to construct things from a scheduler and an argument.
pub trait WithScheduler<Sch, Arg>
where
    Sch: Scheduler,
{
    fn with_scheduler(sch: Sch, arg: Arg) -> Self;
}
