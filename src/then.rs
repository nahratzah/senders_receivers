use crate::errors::{Error, IsTuple};
use crate::functor::{Closure, Functor};
use crate::traits::{BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender};
use core::ops::BitOr;
use std::marker::PhantomData;

pub struct Then<FnType, Out, ArgTuple>
where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<FnType, Out, ArgTuple> Then<FnType, Out, ArgTuple>
where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    pub fn new_err(fn_impl: FnType) -> Then<FnType, Out, ArgTuple> {
        Then {
            fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
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
    pub fn new_fn_err(fn_impl: FnType) -> ClosureThen<FnType, Out, ArgTuple> {
        Self::new_err(Closure::new(fn_impl))
    }
}

type NoErrThen<FunctorType, Out, ArgTuple> =
    Then<NoErrFunctor<FunctorType, Out, ArgTuple>, Out, ArgTuple>;

struct NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    functor: FunctorType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<FunctorType, Out, ArgTuple> Functor<ArgTuple> for NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    type Output = Result<Out, Error>;

    fn tuple_invoke(self, args: ArgTuple) -> Self::Output {
        Ok(self.functor.tuple_invoke(args))
    }
}

impl<FnImpl, Out, ArgTuple> NoErrThen<FnImpl, Out, ArgTuple>
where
    FnImpl: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    pub fn new(fn_impl: FnImpl) -> NoErrThen<FnImpl, Out, ArgTuple> {
        let fn_impl = NoErrFunctor {
            functor: fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
        };
        Self::new_err(fn_impl)
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
            phantom2: PhantomData,
        }
    }
}

impl<NestedSender, FnType, Out: IsTuple, BindSenderImpl> BitOr<BindSenderImpl>
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
    phantom2: PhantomData<Out>,
}

impl<NestedSender, FnType, Out> TypedSender for ThenSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    Out: IsTuple,
{
    type Value = Out;

    fn connect<ReceiverImpl>(self, receiver: ReceiverImpl) -> impl OperationState
    where
        ReceiverImpl: ReceiverOf<Out>,
    {
        let wrapped_receiver = ThenWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
        };

        self.nested.connect(wrapped_receiver)
    }
}

struct ThenWrappedReceiver<ReceiverImpl, FnType, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<ReceiverImpl, FnType, ArgTuple, Out> Receiver
    for ThenWrappedReceiver<ReceiverImpl, FnType, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
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

impl<ReceiverImpl, FnType, ArgTuple, Out> ReceiverOf<ArgTuple>
    for ThenWrappedReceiver<ReceiverImpl, FnType, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    fn set_value(self, values: ArgTuple) {
        match self.fn_impl.tuple_invoke(values) {
            Ok(v) => self.nested.set_value(v),
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
