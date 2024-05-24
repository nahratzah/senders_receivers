use crate::errors::{Error, IsTuple};
use crate::functor::{Closure, Functor, NoErrFunctor};
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use core::ops::BitOr;
use std::marker::PhantomData;

/// A then operation takes the current `value` signal, and transforms it in some way.
/// The result of the function is the new `value` signal.
///
/// To create a Then [Sender] from a [function/closure](FnOnce), use:
/// ```
/// use senders_receivers::{Error, Just, Then, sync_wait};
///
/// // If using a function that returns a tuple:
/// let myFn = |(x, y, z): (i32, i32, i32)| (x + y + z,);
/// let sender = Just::new((1, 2, 3)) | Then::new_fn(myFn);
/// assert_eq!(
///     (6,),
///     sync_wait(sender).unwrap().unwrap());
///
/// // If using a function that returns a Result:
/// let myFn = |(x,)| -> Result<(i32,), Error> { Ok((x,)) };
/// let sender = Just::new((17,)) | Then::new_fn_err(myFn);  // if function returns a result
/// assert_eq!(
///     (17,),
///     sync_wait(sender).unwrap().unwrap());
/// ```
///
/// You probably don't want the non `_fn` versions of those functions:
/// those operate on functors.
pub struct Then<FnType, Out, ArgTuple>
where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    fn_impl: FnType,
    phantom: PhantomData<fn(ArgTuple) -> Out>,
}

impl<FnType, Out, ArgTuple> Then<FnType, Out, ArgTuple>
where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    /// Create a new Then operation, using a [Functor] that returns an [Result].
    pub fn new_err(fn_impl: FnType) -> Then<FnType, Out, ArgTuple> {
        Then {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureThen<FnType, Out, ArgTuple> =
    Then<Closure<FnType, Result<Out, Error>, ArgTuple>, Out, ArgTuple>;

impl<FnType, ArgTuple, Out> ClosureThen<FnType, Out, ArgTuple>
where
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    /// Create a new Then operation, using the specified [function](FnOnce).
    /// The function must return a [Result].
    /// If the returned result holds an error, that'll be propagated via the error signal.
    pub fn new_fn_err(fn_impl: FnType) -> ClosureThen<FnType, Out, ArgTuple> {
        Self::new_err(Closure::new(fn_impl))
    }
}

type NoErrThen<FunctorType, Out, ArgTuple> =
    Then<NoErrFunctor<FunctorType, Out, ArgTuple>, Out, ArgTuple>;

impl<FnImpl, Out, ArgTuple> NoErrThen<FnImpl, Out, ArgTuple>
where
    FnImpl: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    /// Create a new then operation, from a [Functor].
    /// The functor should return a [tuple](IsTuple).
    pub fn new(fn_impl: FnImpl) -> NoErrThen<FnImpl, Out, ArgTuple> {
        Self::new_err(NoErrFunctor::new(fn_impl))
    }
}

type NoErrClosureThen<FnImpl, Out, ArgTuple> =
    NoErrThen<Closure<FnImpl, Out, ArgTuple>, Out, ArgTuple>;

impl<FnImpl, Out, ArgTuple> NoErrClosureThen<FnImpl, Out, ArgTuple>
where
    FnImpl: FnOnce(ArgTuple) -> Out,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    /// Create a new then operation, from a [function/closure](FnOnce).
    /// The function/closure should return a [tuple](IsTuple).
    pub fn new_fn(fn_impl: FnImpl) -> NoErrClosureThen<FnImpl, Out, ArgTuple> {
        Self::new(Closure::new(fn_impl))
    }
}

impl<FnType, Out: IsTuple, ArgTuple: IsTuple> Sender for Then<FnType, Out, ArgTuple> where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>
{
}

impl<FnType, Out, NestedSender> BindSender<NestedSender>
    for Then<FnType, Out, <NestedSender as TypedSender>::Value>
where
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    NestedSender::Value: IsTuple,
    Out: IsTuple,
{
    type Output = ThenSender<NestedSender, FnType, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        ThenSender {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

impl<NestedSender, FnType, Out, BindSenderImpl> BitOr<BindSenderImpl>
    for ThenSender<NestedSender, FnType, Out>
where
    BindSenderImpl: BindSender<ThenSender<NestedSender, FnType, Out>>,
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    NestedSender::Value: IsTuple,
    Out: IsTuple,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ThenSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    Out: IsTuple,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<fn() -> Out>,
}

impl<NestedSender, FnType, Out> TypedSender for ThenSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    Out: IsTuple,
{
    type Value = Out;
    type Scheduler = NestedSender::Scheduler;
}

impl<ReceiverImpl, NestedSender, FnType, Out> TypedSenderConnect<ReceiverImpl>
    for ThenSender<NestedSender, FnType, Out>
where
    ReceiverImpl: ReceiverOf<Self::Scheduler, Out>,
    NestedSender: TypedSender
        + TypedSenderConnect<
            ThenWrappedReceiver<
                ReceiverImpl,
                FnType,
                Self::Scheduler,
                Out,
                <NestedSender as TypedSender>::Value,
            >,
        >,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    Out: IsTuple,
{
    fn connect(self, receiver: ReceiverImpl) -> impl OperationState {
        let wrapped_receiver = ThenWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        };

        self.nested.connect(wrapped_receiver)
    }
}

struct ThenWrappedReceiver<ReceiverImpl, FnType, Sch, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Sch, Out>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    Sch: Scheduler,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom: PhantomData<fn(Sch, ArgTuple) -> Out>,
}

impl<ReceiverImpl, FnType, Sch, ArgTuple, Out> Receiver
    for ThenWrappedReceiver<ReceiverImpl, FnType, Sch, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Sch, Out>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    Sch: Scheduler,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<ReceiverImpl, FnType, Sch, ArgTuple, Out> ReceiverOf<Sch, ArgTuple>
    for ThenWrappedReceiver<ReceiverImpl, FnType, Sch, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Sch, Out>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    Sch: Scheduler,
    ArgTuple: IsTuple,
    Out: IsTuple,
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
    use crate::errors::{Error, ErrorForTesting};
    use crate::just::Just;
    use crate::just_error::JustError;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(Just::new((4, 5, 6)) | Then::new_fn(|(x, y, z)| (x + 2, y + 2, z + 2)))
                .expect("should succeed")
        )
    }

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(
                Just::new((4, 5, 6)) | Then::new_fn_err(|(x, y, z)| Ok((x + 2, y + 2, z + 2)))
            )
            .expect("should succeed")
        )
    }

    #[test]
    fn errors_from_preceding_sender_are_propagated() {
        match sync_wait(
            JustError::<()>::new(Box::new(ErrorForTesting::from("error")))
                | Then::new_fn(|()| -> (i32, i32) {
                    panic!("expect this function to not be invoked")
                }),
        ) {
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
        match sync_wait(
            Just::new(())
                | Then::new_fn_err(|()| -> Result<(), Error> {
                    Err(Box::new(ErrorForTesting::from("error")))
                }),
        ) {
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
