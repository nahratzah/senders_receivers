use crate::errors::Error;
use crate::io::EnableDefaultIO;
use crate::just::Just;
use crate::just_done::JustDone;
use crate::just_error::JustError;
use crate::scope::{ScopeWrap, ScopeWrapSend};
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use crate::tuple::Tuple;
use std::marker::PhantomData;
use std::ops::BitOr;
use threadpool::ThreadPool;

/// Schedulers are things that can do work.
/// All sender chains run on a scheduler.
///
/// Most of the time, you'll use a scheduler in combination with [Transfer](crate::Transfer).
/// But a scheduler can be used directly (which is what [Transfer](crate::Transfer) does under the hood).
/// Unfortunately, I can't make the `|` operator work for [Scheduler::Sender], so we must use the [`BindSender::bind`] function instead.
/// ```
/// use senders_receivers::{BindSender, Scheduler, Then, SyncWaitSend};
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
///     sender.sync_wait_send().unwrap().unwrap().0);
/// ```
pub trait Scheduler: Eq + Clone + 'static {
    /// Mark if the scheduler may block the caller, when started.
    /// If this is `false`, you are guaranteed that the sender will complete independently from the operation-state start method.
    /// But if it returns `true`, the scheduler will complete before returning from the operation-state start method.
    const EXECUTION_BLOCKS_CALLER: bool;
    /// The scheduler that's passed on along the value signal.
    /// Most of the time, this would be the same as this.
    ///
    /// But if for example an embarrisingly-parallel scheduler is used,
    /// the LocalScheduler would represent the scheduler bound to the thread that was selected.
    type LocalScheduler: Scheduler;

    /// The [TypedSender] returned by the scheduler.
    type Sender: for<'a> TypedSender<Scheduler = Self::LocalScheduler, Value = ()>;

    /// Create a [Self::Sender] that'll run on this scheduler.
    fn schedule(&self) -> Self::Sender;

    /// Create a sender that'll run on this scheduler, that produces a value signal.
    fn schedule_value<'a, Tpl: 'a + Tuple>(&self, values: Tpl) -> Just<'a, Self, Tpl> {
        Just::with_scheduler(self.clone(), values)
    }

    /// Create a sender associated with this scheduler, that produces an error signal.
    fn schedule_error<Tpl: Tuple>(&self, error: Error) -> JustError<Self, Tpl> {
        JustError::<Self, Tpl>::new(error)
    }

    /// Create a sender associated with this scheduler, that produces a done signal.
    fn schedule_done<Tpl: Tuple>(&self) -> JustDone<Self, Tpl> {
        JustDone::<Self, Tpl>::new()
    }

    /// Create a scheduler, that won't reschedule immediately, but instead reschedule on the first ehm... reschedule.
    ///
    /// Use these in [LetValue](crate::let_value::LetValue) when you're not switching scheduler:
    /// ```
    /// use senders_receivers::{Scheduler, LetValue, StartDetached};
    /// use senders_receivers::refs;
    /// use threadpool::ThreadPool;
    ///
    /// let pool = ThreadPool::with_name("example".into(), 1);
    /// (
    ///     pool.schedule()
    ///     | LetValue::from(|sch: ThreadPool, _: refs::ScopedRefMut<(), refs::NoSendState>| {
    ///         // Since we are already running in sch, we don't want a reschedule to happen.
    ///         // By using lazy, we basically tell the code that we're already running on that scheduler,
    ///         // and rescheduling isn't needed.
    ///         sch.lazy().schedule_value((1, 2, 3))
    ///     })).start_detached();
    /// ```
    fn lazy(&self) -> LazyScheduler<Self>
    where
        Self: Scheduler<LocalScheduler = Self>,
    {
        LazyScheduler { sch: self.clone() }
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

impl EnableDefaultIO for ImmediateScheduler {}

pub struct ImmediateSender {}

impl TypedSender for ImmediateSender {
    type Scheduler = ImmediateScheduler;
    type Value = ();
}

impl<'a, ScopeImpl, ReceiverType> TypedSenderConnect<'a, ScopeImpl, ReceiverType>
    for ImmediateSender
where
    ReceiverType: ReceiverOf<
        <ImmediateSender as TypedSender>::Scheduler,
        <ImmediateSender as TypedSender>::Value,
    >,
    ScopeImpl: ScopeWrap<<ImmediateSender as TypedSender>::Scheduler, ReceiverType>,
{
    fn connect<'scope>(self, _: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        ImmediateOperationState {
            phantom: PhantomData,
            receiver,
        }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for ImmediateSender
where
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ImmediateOperationState<'scope, ReceiverType>
where
    ReceiverType: ReceiverOf<ImmediateScheduler, ()> + 'scope,
{
    phantom: PhantomData<&'scope ()>,
    receiver: ReceiverType,
}

impl<'scope, ReceiverType> OperationState<'scope> for ImmediateOperationState<'scope, ReceiverType>
where
    ReceiverType: ReceiverOf<ImmediateScheduler, ()> + 'scope,
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

impl EnableDefaultIO for ThreadPool {}

pub struct ThreadPoolSender {
    pool: ThreadPool,
}

impl TypedSender for ThreadPoolSender {
    type Value = ();
    type Scheduler = ThreadPool;
}

impl<'a, ScopeImpl, ReceiverType> TypedSenderConnect<'a, ScopeImpl, ReceiverType>
    for ThreadPoolSender
where
    ReceiverType: Send
        + ReceiverOf<
            <ThreadPoolSender as TypedSender>::Scheduler,
            <ThreadPoolSender as TypedSender>::Value,
        >,
    ScopeImpl: ScopeWrapSend<<ThreadPoolSender as TypedSender>::Scheduler, ReceiverType>,
{
    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        receiver: ReceiverType,
    ) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        ThreadPoolOperationState {
            pool: self.pool,
            receiver: scope.wrap_send(receiver),
            phantom: PhantomData,
        }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for ThreadPoolSender
where
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ThreadPoolOperationState<'a, Receiver>
where
    Receiver: ReceiverOf<ThreadPool, ()> + Send + 'static,
{
    phantom: PhantomData<&'a i32>,
    pool: ThreadPool,
    receiver: Receiver,
}

impl<'a, Receiver> OperationState<'a> for ThreadPoolOperationState<'a, Receiver>
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
    /// Create a new instance of Self, using the given scheduler.
    fn with_scheduler(sch: Sch, arg: Arg) -> Self;
}

/// A lazy scheduler is a scheduler, that doesn't transfer immediately.
///
/// It is used when you already are running on the desired scheduler,
/// but need to re-use it, in for example [LetValue].
#[derive(Clone, Eq, PartialEq)]
pub struct LazyScheduler<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    sch: Sch,
}

impl<Sch> Scheduler for LazyScheduler<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    const EXECUTION_BLOCKS_CALLER: bool = Sch::EXECUTION_BLOCKS_CALLER;
    type LocalScheduler = Sch;
    type Sender = LazySchedulerTS<Sch>;

    fn schedule(&self) -> Self::Sender {
        Self::Sender {
            sch: self.sch.clone(),
        }
    }
}

impl<Sch> EnableDefaultIO for LazyScheduler<Sch> where
    Sch: Scheduler<LocalScheduler = Sch> + EnableDefaultIO
{
}

pub struct LazySchedulerTS<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    sch: Sch,
}

impl<Sch> TypedSender for LazySchedulerTS<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    type Scheduler = Sch;
    type Value = ();
}

impl<'a, ScopeImpl, ReceiverType, Sch> TypedSenderConnect<'a, ScopeImpl, ReceiverType>
    for LazySchedulerTS<Sch>
where
    ReceiverType: ReceiverOf<Sch, ()>,
    Sch: Scheduler<LocalScheduler = Sch>,
    ScopeImpl: ScopeWrap<Sch, ReceiverType>,
{
    fn connect<'scope>(self, _: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        LazySchedulerOperationState {
            sch: self.sch,
            receiver,
            phantom: PhantomData,
        }
    }
}

struct LazySchedulerOperationState<'scope, Sch, ReceiverType>
where
    ReceiverType: ReceiverOf<Sch, ()> + 'scope,
    Sch: Scheduler<LocalScheduler = Sch>,
{
    phantom: PhantomData<&'scope i32>,
    sch: Sch,
    receiver: ReceiverType,
}

impl<'scope, Sch, ReceiverType> OperationState<'scope>
    for LazySchedulerOperationState<'scope, Sch, ReceiverType>
where
    ReceiverType: ReceiverOf<Sch, ()> + 'scope,
    Sch: Scheduler<LocalScheduler = Sch>,
{
    fn start(self) {
        self.receiver.set_value(self.sch, ());
    }
}
