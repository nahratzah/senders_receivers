use crate::errors::{Error, IsTuple, Result};
use crate::functor::{NoArgClosure, NoArgFunctor, NoErrNoArgFunctor};
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use std::convert::From;
use std::marker::PhantomData;
use std::ops::BitOr;

/// Handle a done-signal, by transforming it into a value signal.
///
/// UponDone is initialized with a function, that'll produce the value (or error)
/// that is to be placed at this position in the sender chain.
/// It also has an associated scheduler, which is the scheduler on which it must be scheduled.
///
/// The scheduler and value must match the current output of the signal, propagated via the sender chain.
///
/// The [UponDone::from] implementations use [ImmediateScheduler] as their scheduler.
/// Example:
/// ```
/// use senders_receivers::{ImmediateScheduler, JustDone, UponDone, sync_wait};
///
/// let sender = JustDone::<ImmediateScheduler, (String,)>::default()
///              | UponDone::from(|| (String::from("result"),));
/// assert_eq!(
///     (String::from("result"),),
///     sync_wait(sender).unwrap().unwrap())
/// ```
///
/// If a specific scheduler is required, the [UponDone::with_scheduler] constructor is the one to use:
/// ```
/// use senders_receivers::{ImmediateScheduler, JustDone, UponDone, Transfer, WithScheduler, sync_wait_send};
/// use threadpool::ThreadPool;
///
/// let pool = ThreadPool::with_name("example".into(), 1);
/// let sender = JustDone::<ImmediateScheduler, (String,)>::default()
///              | Transfer::new(pool.clone())
///              | UponDone::with_scheduler(pool.clone(), || (String::from("result"),));
/// assert_eq!(
///     (String::from("result"),),
///     sync_wait_send(sender).unwrap().unwrap())
/// ```
pub struct UponDone<FnType, Sch, Out>
where
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    fn_impl: FnType,
    sch: Sch,
    phantom: PhantomData<fn() -> Out>,
}

impl<FnType, Out> From<FnType> for UponDone<FnType, ImmediateScheduler, Out>
where
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self {
            fn_impl,
            sch: ImmediateScheduler::default(),
            phantom: PhantomData,
        }
    }
}

type ClosureUponDone<FnType, Sch, Out> = UponDone<NoArgClosure<FnType, Result<Out>>, Sch, Out>;

impl<FnType, Out> From<FnType> for ClosureUponDone<FnType, ImmediateScheduler, Out>
where
    FnType: FnOnce() -> Result<Out>,
    Out: IsTuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoArgClosure::new(fn_impl))
    }
}

type NoErrUponDone<FunctorType, Sch, Out> = UponDone<NoErrNoArgFunctor<FunctorType, Out>, Sch, Out>;

impl<FnImpl, Out> From<FnImpl> for NoErrUponDone<FnImpl, ImmediateScheduler, Out>
where
    FnImpl: NoArgFunctor<Output = Out>,
    Out: IsTuple,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(NoErrNoArgFunctor::new(fn_impl))
    }
}

type NoErrClosureUponDone<FnImpl, Sch, Out> = NoErrUponDone<NoArgClosure<FnImpl, Out>, Sch, Out>;

impl<FnImpl, Out> From<FnImpl> for NoErrClosureUponDone<FnImpl, ImmediateScheduler, Out>
where
    FnImpl: FnOnce() -> Out,
    Out: IsTuple,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(NoArgClosure::new(fn_impl))
    }
}

impl<FnType, Sch, Out> WithScheduler<Sch, FnType> for UponDone<FnType, Sch, Out>
where
    FnType: NoArgFunctor<Output = Result<Out>>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn with_scheduler(sch: Sch, fn_impl: FnType) -> Self {
        Self {
            fn_impl,
            sch,
            phantom: PhantomData,
        }
    }
}

impl<FnType, Sch, Out> WithScheduler<Sch, FnType> for ClosureUponDone<FnType, Sch, Out>
where
    FnType: FnOnce() -> Result<Out>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn with_scheduler(sch: Sch, fn_impl: FnType) -> Self {
        Self::with_scheduler(sch, NoArgClosure::new(fn_impl))
    }
}

impl<FnImpl, Sch, Out> WithScheduler<Sch, FnImpl> for NoErrUponDone<FnImpl, Sch, Out>
where
    FnImpl: NoArgFunctor<Output = Out>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn with_scheduler(sch: Sch, fn_impl: FnImpl) -> Self {
        Self::with_scheduler(sch, NoErrNoArgFunctor::new(fn_impl))
    }
}

impl<FnImpl, Sch, Out> WithScheduler<Sch, FnImpl> for NoErrClosureUponDone<FnImpl, Sch, Out>
where
    FnImpl: FnOnce() -> Out,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn with_scheduler(sch: Sch, fn_impl: FnImpl) -> Self {
        Self::with_scheduler(sch, NoArgClosure::new(fn_impl))
    }
}

impl<FnType, Sch, Out> Sender for UponDone<FnType, Sch, Out>
where
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
}

impl<FnType, Sch, Out, NestedSender> BindSender<NestedSender> for UponDone<FnType, Sch, Out>
where
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    type Output = UponDoneTS<NestedSender, FnType, Sch, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        UponDoneTS {
            nested,
            fn_impl: self.fn_impl,
            sch: self.sch,
            phantom: PhantomData,
        }
    }
}

pub struct UponDoneTS<NestedSender, FnType, Sch, Out>
where
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    nested: NestedSender,
    fn_impl: FnType,
    sch: Sch,
    phantom: PhantomData<fn() -> Out>,
}

impl<NestedSender, FnType, Sch, Out> TypedSender for UponDoneTS<NestedSender, FnType, Sch, Out>
where
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = Out;
}

impl<ReceiverType, NestedSender, FnType, Sch, Out> TypedSenderConnect<ReceiverType>
    for UponDoneTS<NestedSender, FnType, Sch, Out>
where
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Out>,
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>
        + TypedSenderConnect<ReceiverWrapper<ReceiverType, FnType, Sch, Out>>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
    Sch::Sender: TypedSenderConnect<DoneReceiver<ReceiverType, FnType, Sch::LocalScheduler, Out>>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        let receiver: ReceiverWrapper<ReceiverType, FnType, Sch, Out> = ReceiverWrapper {
            nested: receiver,
            fn_impl: self.fn_impl,
            sch: self.sch,
            phantom: PhantomData,
        };
        self.nested.connect(receiver)
    }
}

impl<NestedSender, FnType, Sch, Out, BindSenderImpl> BitOr<BindSenderImpl>
    for UponDoneTS<NestedSender, FnType, Sch, Out>
where
    BindSenderImpl: BindSender<UponDoneTS<NestedSender, FnType, Sch, Out>>,
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Sch: Scheduler,
    Sch::Sender: TypedSenderConnect<DoneReceiver<NestedReceiver, FnType, Sch::LocalScheduler, Out>>,
    Out: IsTuple,
{
    nested: NestedReceiver,
    fn_impl: FnType,
    sch: Sch,
    phantom: PhantomData<fn() -> Out>,
}

impl<NestedReceiver, FnType, Sch, Out> Receiver
    for ReceiverWrapper<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Sch: Scheduler,
    Sch::Sender: TypedSenderConnect<DoneReceiver<NestedReceiver, FnType, Sch::LocalScheduler, Out>>,
    Out: IsTuple,
{
    fn set_done(self) {
        self.sch
            .schedule()
            .connect(
                DoneReceiver::<NestedReceiver, FnType, Sch::LocalScheduler, Out> {
                    nested: self.nested,
                    fn_impl: self.fn_impl,
                    phantom: PhantomData,
                },
            )
            .start()
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<NestedReceiver, FnType, Sch, Out> ReceiverOf<Sch::LocalScheduler, Out>
    for ReceiverWrapper<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Sch: Scheduler,
    Sch::Sender: TypedSenderConnect<DoneReceiver<NestedReceiver, FnType, Sch::LocalScheduler, Out>>,
    Out: IsTuple,
{
    fn set_value(self, sch: Sch::LocalScheduler, v: Out) {
        self.nested.set_value(sch, v);
    }
}

struct DoneReceiver<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch, Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Sch: Scheduler,
    Out: IsTuple,
{
    nested: NestedReceiver,
    fn_impl: FnType,
    phantom: PhantomData<fn(Sch) -> Out>,
}

impl<NestedReceiver, FnType, Sch, Out> Receiver for DoneReceiver<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch, Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<NestedReceiver, FnType, Sch, Out> ReceiverOf<Sch, ()>
    for DoneReceiver<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch, Out>,
    FnType: NoArgFunctor<Output = Result<Out>>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn set_value(self, sch: Sch, _: ()) {
        match self.fn_impl.tuple_invoke() {
            Ok(v) => self.nested.set_value(sch, v),
            Err(e) => self.nested.set_error(e),
        }
    }
}
