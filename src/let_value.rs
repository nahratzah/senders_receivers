use crate::errors::{Error, IsTuple};
use crate::functor::{Closure, Functor};
use crate::traits::{BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender};
use core::ops::BitOr;
use std::marker::PhantomData;

pub struct LetValue<FnType, Out, ArgTuple>
where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<FnType, Out, ArgTuple> LetValue<FnType, Out, ArgTuple>
where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new_err(fn_impl: FnType) -> LetValue<FnType, Out, ArgTuple> {
        LetValue {
            fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
        }
    }
}

type ClosureLetValue<FnType, Out, ArgTuple> =
    LetValue<Closure<FnType, Result<Out, Error>, ArgTuple>, Out, ArgTuple>;

impl<FnType, Out, ArgTuple> ClosureLetValue<FnType, Out, ArgTuple>
where
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new_fn_err(fn_impl: FnType) -> ClosureLetValue<FnType, Out, ArgTuple> {
        Self::new_err(Closure::new(fn_impl))
    }
}

type NoErrLetValue<FunctorType, Out, ArgTuple> =
    LetValue<NoErrFunctor<FunctorType, Out, ArgTuple>, Out, ArgTuple>;

struct NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    functor: FunctorType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<FunctorType, Out, ArgTuple> Functor<ArgTuple> for NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    type Output = Result<Out, Error>;

    fn tuple_invoke(self, args: ArgTuple) -> Self::Output {
        Ok(self.functor.tuple_invoke(args))
    }
}

impl<FnImpl, Out, ArgTuple> NoErrLetValue<FnImpl, Out, ArgTuple>
where
    FnImpl: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new(fn_impl: FnImpl) -> NoErrLetValue<FnImpl, Out, ArgTuple> {
        let fn_impl = NoErrFunctor {
            functor: fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
        };
        Self::new_err(fn_impl)
    }
}

type NoErrClosureLetValue<FnImpl, Out, ArgTuple> =
    NoErrLetValue<Closure<FnImpl, Out, ArgTuple>, Out, ArgTuple>;

impl<FnImpl, Out, ArgTuple> NoErrClosureLetValue<FnImpl, Out, ArgTuple>
where
    FnImpl: FnOnce(ArgTuple) -> Out,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new_fn(fn_impl: FnImpl) -> NoErrClosureLetValue<FnImpl, Out, ArgTuple> {
        Self::new(Closure::new(fn_impl))
    }
}

impl<FnType, Out: TypedSender, ArgTuple: IsTuple> Sender for LetValue<FnType, Out, ArgTuple> where
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>
{
}

impl<FnType, Out, NestedSender> BindSender<NestedSender>
    for LetValue<FnType, Out, <NestedSender as TypedSender>::Value>
where
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    NestedSender::Value: IsTuple,
    Out: TypedSender,
{
    type Output = LetValueSender<NestedSender, FnType, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        LetValueSender {
            nested,
            fn_impl: self.fn_impl,
            phantom2: PhantomData,
        }
    }
}

impl<NestedSender, FnType, Out: TypedSender, BindSenderImpl> BitOr<BindSenderImpl>
    for LetValueSender<NestedSender, FnType, Out>
where
    BindSenderImpl: BindSender<LetValueSender<NestedSender, FnType, Out>>,
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    NestedSender::Value: IsTuple,
    Out: TypedSender,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct LetValueSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    Out: TypedSender,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom2: PhantomData<Out>,
}

impl<NestedSender, FnType, Out> TypedSender for LetValueSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: Functor<NestedSender::Value, Output = Result<Out, Error>>,
    Out: TypedSender,
{
    type Value = Out::Value;

    fn connect<ReceiverImpl>(self, receiver: ReceiverImpl) -> impl OperationState
    where
        ReceiverImpl: ReceiverOf<Out::Value>,
    {
        let wrapped_receiver = LetValueWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
        };
        self.nested.connect(wrapped_receiver)
    }
}

struct LetValueWrappedReceiver<ReceiverImpl, FnType, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Value>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<ReceiverImpl, FnType, Out, ArgTuple> Receiver
    for LetValueWrappedReceiver<ReceiverImpl, FnType, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Value>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<ReceiverImpl, FnType, Out, ArgTuple> ReceiverOf<ArgTuple>
    for LetValueWrappedReceiver<ReceiverImpl, FnType, Out, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Value>,
    FnType: Functor<ArgTuple, Output = Result<Out, Error>>,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn set_value(self, values: ArgTuple) {
        match self.fn_impl.tuple_invoke(values) {
            Ok(sender) => {
                let nested_state = sender.connect(self.nested);
                nested_state.start()
            }
            Err(e) => self.nested.set_error(e),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::LetValue;
    use crate::errors::Error;
    use crate::just::Just;
    use crate::just_error::JustError;
    use crate::sync_wait::sync_wait;
    use std::any::TypeId;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(
                Just::new((6,))
                    | LetValue::new_fn(|(x,)| {
                        assert_eq!(x, 6);
                        Just::new((x, 7, 8))
                    })
            )
            .expect("should succeed")
        )
    }

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(Just::new((6,)) | LetValue::new_fn_err(|(x,)| Ok(Just::new((x, 7, 8)))))
                .expect("should succeed")
        )
    }

    #[test]
    fn errors_from_preceding_sender_are_propagated() {
        match sync_wait(
            JustError::<()>::new(Box::new(String::from("error")))
                | LetValue::new_fn(|()| -> Just<(i32,)> {
                    panic!("expect this function to not be invoked")
                }),
        ) {
            Ok(_) => panic!("expected an error"),
            Err(e) => {
                assert_eq!(TypeId::of::<String>(), (&*e).type_id());
                assert_eq!(String::from("error"), *e.downcast_ref::<String>().unwrap());
            }
        }
    }

    #[test]
    fn errors_from_functor_are_propagated() {
        match sync_wait(
            Just::new(())
                | LetValue::new_fn_err(|()| -> Result<Just<(i32,)>, Error> {
                    Err(Box::new(String::from("error")))
                }),
        ) {
            Ok(_) => panic!("expected an error"),
            Err(e) => {
                assert_eq!(TypeId::of::<String>(), (&*e).type_id());
                assert_eq!(String::from("error"), *e.downcast_ref::<String>().unwrap());
            }
        }
    }

    #[test]
    fn errors_from_nested_sender_are_propagated() {
        // nested_sender refers to the sender returned by the functor.
        match sync_wait(
            Just::new(())
                | LetValue::new_fn(|()| JustError::<()>::new(Box::new(String::from("error")))),
        ) {
            Ok(_) => panic!("expected an error"),
            Err(e) => {
                assert_eq!(TypeId::of::<String>(), (&*e).type_id());
                assert_eq!(String::from("error"), *e.downcast_ref::<String>().unwrap());
            }
        }
    }
}
