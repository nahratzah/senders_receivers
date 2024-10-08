use crate::errors::{Error, Result};
use crate::functor::{Closure, Functor, NoErrFunctor};
use crate::scheduler::Scheduler;
use crate::stop_token::StopToken;
use crate::traits::{BindSender, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect};
use crate::tuple::Tuple;
use std::marker::PhantomData;
use std::ops::BitOr;

/// A then operation takes the current `value` signal, and transforms it in some way.
/// The result of the function is the new `value` signal.
///
/// To create a Then [Sender] from a [function/closure](FnOnce), use:
/// ```
/// use senders_receivers::{Error, Just, Then, SyncWait};
///
/// // If using a function that returns a tuple:
/// let myFn = |(x, y, z): (i32, i32, i32)| (x + y + z,);
/// let sender = Just::from((1, 2, 3)) | Then::from(myFn);
/// assert_eq!(
///     (6,),
///     sender.sync_wait().unwrap().unwrap());
///
/// // If using a function that returns a Result:
/// let myFn = |(x,)| -> Result<(i32,), Error> { Ok((x,)) };
/// let sender = Just::from((17,)) | Then::from(myFn);  // if function returns a result
/// assert_eq!(
///     (17,),
///     sender.sync_wait().unwrap().unwrap());
/// ```
///
/// You probably don't want the non `_fn` versions of those functions:
/// those operate on functors.
pub struct Then<'a, FnType, Out, ArgTuple>
where
    FnType: 'a + Functor<'a, ArgTuple, Output = Result<Out>>,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    fn_impl: FnType,
    phantom: PhantomData<&'a fn(ArgTuple) -> Out>,
}

impl<'a, FnType, Out, ArgTuple> From<FnType> for Then<'a, FnType, Out, ArgTuple>
where
    FnType: 'a + Functor<'a, ArgTuple, Output = Result<Out>>,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    /// Create a new Then operation, using a [Functor] that returns an [Result].
    fn from(fn_impl: FnType) -> Self {
        Then {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureThen<'a, FnType, Out, ArgTuple> =
    Then<'a, Closure<'a, FnType, Result<Out>, ArgTuple>, Out, ArgTuple>;

impl<'a, FnType, ArgTuple, Out> From<FnType> for ClosureThen<'a, FnType, Out, ArgTuple>
where
    FnType: 'a + FnOnce(ArgTuple) -> Result<Out>,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    /// Create a new Then operation, using the specified [function](FnOnce).
    /// The function must return a [Result].
    /// If the returned result holds an error, that'll be propagated via the error signal.
    fn from(fn_impl: FnType) -> Self {
        Self::from(Closure::new(fn_impl))
    }
}

type NoErrThen<'a, FunctorType, Out, ArgTuple> =
    Then<'a, NoErrFunctor<'a, FunctorType, Out, ArgTuple>, Out, ArgTuple>;

impl<'a, FnImpl, Out, ArgTuple> From<FnImpl> for NoErrThen<'a, FnImpl, Out, ArgTuple>
where
    FnImpl: 'a + Functor<'a, ArgTuple, Output = Out>,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    /// Create a new then operation, from a [Functor].
    /// The functor should return a [tuple](Tuple).
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(NoErrFunctor::new(fn_impl))
    }
}

type NoErrClosureThen<'a, FnImpl, Out, ArgTuple> =
    NoErrThen<'a, Closure<'a, FnImpl, Out, ArgTuple>, Out, ArgTuple>;

impl<'a, FnImpl, Out, ArgTuple> From<FnImpl> for NoErrClosureThen<'a, FnImpl, Out, ArgTuple>
where
    FnImpl: 'a + FnOnce(ArgTuple) -> Out,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    /// Create a new then operation, from a [function/closure](FnOnce).
    /// The function/closure should return a [tuple](Tuple).
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(Closure::new(fn_impl))
    }
}

impl<'a, FnType, Out: Tuple, ArgTuple: Tuple> Sender for Then<'a, FnType, Out, ArgTuple> where
    FnType: 'a + Functor<'a, ArgTuple, Output = Result<Out>>
{
}

impl<'a, FnType, Out, NestedSender> BindSender<NestedSender>
    for Then<'a, FnType, Out, <NestedSender as TypedSender>::Value>
where
    NestedSender: TypedSender,
    FnType: 'a + Functor<'a, NestedSender::Value, Output = Result<Out>>,
    NestedSender::Value: Tuple,
    Out: 'a + Tuple,
{
    type Output = ThenSender<'a, NestedSender, FnType, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        ThenSender {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

impl<'a, NestedSender, FnType, Out, BindSenderImpl> BitOr<BindSenderImpl>
    for ThenSender<'a, NestedSender, FnType, Out>
where
    BindSenderImpl: BindSender<ThenSender<'a, NestedSender, FnType, Out>>,
    NestedSender: TypedSender,
    FnType: 'a + Functor<'a, NestedSender::Value, Output = Result<Out>>,
    NestedSender::Value: Tuple,
    Out: 'a + Tuple,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ThenSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: 'a + Functor<'a, NestedSender::Value, Output = Result<Out>>,
    Out: 'a + Tuple,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
}

impl<'a, NestedSender, FnType, Out> TypedSender for ThenSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: 'a + Functor<'a, NestedSender::Value, Output = Result<Out>>,
    Out: 'a + Tuple,
{
    type Value = Out;
    type Scheduler = NestedSender::Scheduler;
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverImpl, NestedSender, FnType, Out>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverImpl>
    for ThenSender<'a, NestedSender, FnType, Out>
where
    ReceiverImpl: ReceiverOf<Self::Scheduler, Out>,
    NestedSender: TypedSender
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            StopTokenImpl,
            ThenWrappedReceiver<
                'a,
                ReceiverImpl,
                FnType,
                Self::Scheduler,
                Out,
                <NestedSender as TypedSender>::Value,
            >,
        >,
    NestedSender::Value: 'a,
    FnType: 'a + Functor<'a, NestedSender::Value, Output = Result<Out>>,
    Out: 'a + Tuple,
    StopTokenImpl: StopToken,
{
    type Output<'scope> = NestedSender::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverImpl: 'scope;

    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        stop_token: StopTokenImpl,
        receiver: ReceiverImpl,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverImpl: 'scope,
    {
        let wrapped_receiver = ThenWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        };

        self.nested.connect(scope, stop_token, wrapped_receiver)
    }
}

pub struct ThenWrappedReceiver<'a, ReceiverImpl, FnType, Sch, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Sch, Out>,
    FnType: 'a + Functor<'a, ArgTuple, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn(Sch, ArgTuple) -> Out>,
}

impl<'a, ReceiverImpl, FnType, Sch, ArgTuple, Out> Receiver
    for ThenWrappedReceiver<'a, ReceiverImpl, FnType, Sch, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Sch, Out>,
    FnType: 'a + Functor<'a, ArgTuple, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<'a, ReceiverImpl, FnType, Sch, ArgTuple, Out> ReceiverOf<Sch, ArgTuple>
    for ThenWrappedReceiver<'a, ReceiverImpl, FnType, Sch, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Sch, Out>,
    FnType: 'a + Functor<'a, ArgTuple, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    ArgTuple: Tuple,
    Out: 'a + Tuple,
{
    fn set_value(self, sch: Sch, values: ArgTuple) {
        match self.fn_impl.tuple_invoke(values) {
            Ok(v) => self.nested.set_value(sch, v),
            Err(e) => self.nested.set_error(e),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Then;
    use crate::errors::{new_error, ErrorForTesting, Result};
    use crate::just::Just;
    use crate::just_error::JustError;
    use crate::scheduler::ImmediateScheduler;
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((6, 7, 8)),
            (Just::from((4, 5, 6)) | Then::from(|(x, y, z)| (x + 2, y + 2, z + 2)))
                .sync_wait()
                .expect("should succeed")
        )
    }

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            Some((6, 7, 8)),
            (Just::from((4, 5, 6)) | Then::from(|(x, y, z)| Ok((x + 2, y + 2, z + 2))))
                .sync_wait()
                .expect("should succeed")
        )
    }

    #[test]
    fn errors_from_preceding_sender_are_propagated() {
        match (JustError::<ImmediateScheduler, ()>::from(new_error(ErrorForTesting::from("error")))
            | Then::from(|()| -> (i32, i32) { panic!("expect this function to not be invoked") }))
        .sync_wait()
        {
            Ok(_) => panic!("expected an error"),
            Err(e) => {
                assert_eq!(
                    ErrorForTesting::from("error"),
                    *e.downcast_ref::<ErrorForTesting>().unwrap()
                );
            }
        }
    }

    #[test]
    fn errors_from_functor_are_propagated() {
        match (Just::from(())
            | Then::from(|()| -> Result<()> { Err(Box::new(ErrorForTesting::from("error"))) }))
        .sync_wait()
        {
            Ok(_) => panic!("expected an error"),
            Err(e) => {
                assert_eq!(
                    ErrorForTesting::from("error"),
                    *e.downcast_ref::<ErrorForTesting>().unwrap()
                );
            }
        }
    }
}
