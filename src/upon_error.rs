use crate::errors::{Error, IsTuple, Result};
use crate::functor::{Closure, Functor, NoErrFunctor};
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use std::convert::From;
use std::marker::PhantomData;
use std::ops::BitOr;

/// Handle an error-signal, by transforming an [Error] into a value signal.
///
/// UponError is initialized with a function, that'll produce a value (or error)
/// that is to be placed at this position in the sender chain.
/// It also has an associated scheduler, which is the scheduler on which it must be scheduled.
///
/// The scheduler and value must match the current output of the signal, propagated via the sender chain.
///
/// The [UponError::from] implementations use [ImmediateScheduler] as their scheduler.
/// Example:
/// ```
/// use senders_receivers::{Error, ImmediateScheduler, JustError, UponError, sync_wait, new_error};
/// use std::io;
///
/// let sender = JustError::<ImmediateScheduler, (String,)>::new(new_error(io::Error::new(io::ErrorKind::Other, "oh no!")))
///              | UponError::from(|e: Error| (format!("error: {:?}", e),));
/// println!("result: {}", sync_wait(sender).unwrap().unwrap().0);
/// ```
///
/// If a specific scheduler is required, the [UponError::with_scheduler] constructor is the one to use:
/// ```
/// use senders_receivers::{Error, ImmediateScheduler, JustError, UponError, Transfer, WithScheduler, sync_wait_send, new_error};
/// use std::io;
/// use threadpool::ThreadPool;
///
/// let pool = ThreadPool::with_name("example".into(), 1);
/// let sender = JustError::<ImmediateScheduler, (String,)>::new(new_error(io::Error::new(io::ErrorKind::Other, "oh no!")))
///              | Transfer::new(pool.clone())
///              | UponError::with_scheduler(pool.clone(), |e: Error| (format!("error: {:?}", e),));
/// println!("result: {}", sync_wait_send(sender).unwrap().unwrap().0);
/// ```
///
/// If you wish to handle a specific error only, the way to do that is:
/// ```
/// use senders_receivers::{Error, ImmediateScheduler, JustError, UponError, sync_wait, new_error};
/// use std::io;
///
/// let sender = JustError::<ImmediateScheduler, (String,)>::new(new_error(io::Error::new(io::ErrorKind::Other, "oh no!")))
///              | UponError::from(
///                  |e: Error| {
///                    match e.downcast_ref::<io::Error>() {
///                      Some(error) => Ok((format!("error: {:?}", error),)),
///                      None => Err(e),
///                    }
///              });
/// println!("result: {}", sync_wait(sender).unwrap().unwrap().0);
/// ```
pub struct UponError<FnType, Sch, Out>
where
    FnType: Functor<Error, Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    fn_impl: FnType,
    sch: Sch,
    phantom: PhantomData<fn() -> Out>,
}

impl<FnType, Out> From<FnType> for UponError<FnType, ImmediateScheduler, Out>
where
    FnType: Functor<Error, Output = Result<Out>>,
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

type ClosureUponError<FnType, Sch, Out> = UponError<Closure<FnType, Result<Out>, Error>, Sch, Out>;

impl<FnType, Out> From<FnType> for ClosureUponError<FnType, ImmediateScheduler, Out>
where
    FnType: FnOnce(Error) -> Result<Out>,
    Out: IsTuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(Closure::new(fn_impl))
    }
}

type NoErrUponError<FunctorType, Sch, Out> =
    UponError<NoErrFunctor<FunctorType, Out, Error>, Sch, Out>;

impl<FnImpl, Out> From<FnImpl> for NoErrUponError<FnImpl, ImmediateScheduler, Out>
where
    FnImpl: Functor<Error, Output = Out>,
    Out: IsTuple,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(NoErrFunctor::new(fn_impl))
    }
}

type NoErrClosureUponError<FnImpl, Sch, Out> =
    NoErrUponError<Closure<FnImpl, Out, Error>, Sch, Out>;

impl<FnImpl, Out> From<FnImpl> for NoErrClosureUponError<FnImpl, ImmediateScheduler, Out>
where
    FnImpl: FnOnce(Error) -> Out,
    Out: IsTuple,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(Closure::new(fn_impl))
    }
}

impl<FnType, Sch, Out> WithScheduler<Sch, FnType> for UponError<FnType, Sch, Out>
where
    FnType: Functor<Error, Output = Result<Out>>,
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

impl<FnType, Sch, Out> WithScheduler<Sch, FnType> for ClosureUponError<FnType, Sch, Out>
where
    FnType: FnOnce(Error) -> Result<Out>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn with_scheduler(sch: Sch, fn_impl: FnType) -> Self {
        Self::with_scheduler(sch, Closure::new(fn_impl))
    }
}

impl<FnImpl, Sch, Out> WithScheduler<Sch, FnImpl> for NoErrUponError<FnImpl, Sch, Out>
where
    FnImpl: Functor<Error, Output = Out>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn with_scheduler(sch: Sch, fn_impl: FnImpl) -> Self {
        Self::with_scheduler(sch, NoErrFunctor::new(fn_impl))
    }
}

impl<FnImpl, Sch, Out> WithScheduler<Sch, FnImpl> for NoErrClosureUponError<FnImpl, Sch, Out>
where
    FnImpl: FnOnce(Error) -> Out,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn with_scheduler(sch: Sch, fn_impl: FnImpl) -> Self {
        Self::with_scheduler(sch, Closure::new(fn_impl))
    }
}

impl<FnType, Sch, Out> Sender for UponError<FnType, Sch, Out>
where
    FnType: Functor<Error, Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
}

impl<FnType, Sch, Out, NestedSender> BindSender<NestedSender> for UponError<FnType, Sch, Out>
where
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: Functor<Error, Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    type Output = UponErrorTS<NestedSender, FnType, Sch, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        UponErrorTS {
            nested,
            fn_impl: self.fn_impl,
            sch: self.sch,
            phantom: PhantomData,
        }
    }
}

pub struct UponErrorTS<NestedSender, FnType, Sch, Out>
where
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: Functor<Error, Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    nested: NestedSender,
    fn_impl: FnType,
    sch: Sch,
    phantom: PhantomData<fn() -> Out>,
}

impl<NestedSender, FnType, Sch, Out> TypedSender for UponErrorTS<NestedSender, FnType, Sch, Out>
where
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: Functor<Error, Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = Out;
}

impl<ReceiverType, NestedSender, FnType, Sch, Out> TypedSenderConnect<ReceiverType>
    for UponErrorTS<NestedSender, FnType, Sch, Out>
where
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Out>,
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>
        + TypedSenderConnect<ReceiverWrapper<ReceiverType, FnType, Sch, Out>>,
    FnType: Functor<Error, Output = Result<Out>>,
    Out: IsTuple,
    Sch: Scheduler,
    Sch::Sender: TypedSenderConnect<ErrorReceiver<ReceiverType, FnType, Sch::LocalScheduler, Out>>,
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
    for UponErrorTS<NestedSender, FnType, Sch, Out>
where
    BindSenderImpl: BindSender<UponErrorTS<NestedSender, FnType, Sch, Out>>,
    NestedSender: TypedSender<Scheduler = Sch::LocalScheduler, Value = Out>,
    FnType: Functor<Error, Output = Result<Out>>,
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
    FnType: Functor<Error, Output = Result<Out>>,
    Sch: Scheduler,
    Sch::Sender:
        TypedSenderConnect<ErrorReceiver<NestedReceiver, FnType, Sch::LocalScheduler, Out>>,
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
    FnType: Functor<Error, Output = Result<Out>>,
    Sch: Scheduler,
    Sch::Sender:
        TypedSenderConnect<ErrorReceiver<NestedReceiver, FnType, Sch::LocalScheduler, Out>>,
    Out: IsTuple,
{
    fn set_done(self) {
        self.nested.set_done()
    }

    fn set_error(self, error: Error) {
        self.sch
            .schedule()
            .connect(
                ErrorReceiver::<NestedReceiver, FnType, Sch::LocalScheduler, Out> {
                    nested: self.nested,
                    error,
                    fn_impl: self.fn_impl,
                    phantom: PhantomData,
                },
            )
            .start()
    }
}

impl<NestedReceiver, FnType, Sch, Out> ReceiverOf<Sch::LocalScheduler, Out>
    for ReceiverWrapper<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch::LocalScheduler, Out>,
    FnType: Functor<Error, Output = Result<Out>>,
    Sch: Scheduler,
    Sch::Sender:
        TypedSenderConnect<ErrorReceiver<NestedReceiver, FnType, Sch::LocalScheduler, Out>>,
    Out: IsTuple,
{
    fn set_value(self, sch: Sch::LocalScheduler, v: Out) {
        self.nested.set_value(sch, v);
    }
}

struct ErrorReceiver<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch, Out>,
    FnType: Functor<Error, Output = Result<Out>>,
    Sch: Scheduler,
    Out: IsTuple,
{
    nested: NestedReceiver,
    error: Error,
    fn_impl: FnType,
    phantom: PhantomData<fn(Sch) -> Out>,
}

impl<NestedReceiver, FnType, Sch, Out> Receiver for ErrorReceiver<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch, Out>,
    FnType: Functor<Error, Output = Result<Out>>,
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
    for ErrorReceiver<NestedReceiver, FnType, Sch, Out>
where
    NestedReceiver: ReceiverOf<Sch, Out>,
    FnType: Functor<Error, Output = Result<Out>>,
    Sch: Scheduler,
    Out: IsTuple,
{
    fn set_value(self, sch: Sch, _: ()) {
        match self.fn_impl.tuple_invoke(self.error) {
            Ok(v) => self.nested.set_value(sch, v),
            Err(e) => self.nested.set_error(e),
        }
    }
}
