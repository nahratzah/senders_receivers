use crate::errors::{Error, IsTuple};
use crate::functor::{BiClosure, BiFunctor, NoErrBiFunctor};
use crate::scheduler::Scheduler;
use crate::traits::{BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender};
use core::ops::BitOr;
use std::marker::PhantomData;

/// Create a let-value sender.
///
/// A let-value sender is a sender, which, upon receiving a value, invokes a function.
/// The function returns a new typed sender, which will be played out in this place of the chain.
///
/// Example:
/// ```
/// use senders_receivers::{Just, LetValue, sync_wait};
///
/// // If using a function that returns a sender:
/// let sender = Just::new((String::from("world"),))
///              | LetValue::new_fn(|_, (name,)| {
///                  Just::new((format!("Hello {}!", name),))
///              });
/// assert_eq!(
///     (String::from("Hello world!"),),
///     sync_wait(sender).unwrap().unwrap());
///
/// // If using a function that returns a Result:
/// let sender = Just::new((String::from("world"),))
///              | LetValue::new_fn_err(|_, (name,)| {
///                  Ok(Just::new((format!("Hello {}!", name),)))
///              });
/// assert_eq!(
///     (String::from("Hello world!"),),
///     sync_wait(sender).unwrap().unwrap());
/// ```
pub struct LetValue<FnType, Out, FirstArg, ArgTuple>
where
    FnType: BiFunctor<FirstArg, ArgTuple, Output = Result<Out, Error>>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn_impl: FnType,
    phantom0: PhantomData<FirstArg>,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<FnType, Out, FirstArg, ArgTuple> LetValue<FnType, Out, FirstArg, ArgTuple>
where
    FnType: BiFunctor<FirstArg, ArgTuple, Output = Result<Out, Error>>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new_err(fn_impl: FnType) -> LetValue<FnType, Out, FirstArg, ArgTuple> {
        LetValue {
            fn_impl,
            phantom0: PhantomData,
            phantom1: PhantomData,
            phantom2: PhantomData,
        }
    }
}

type ClosureLetValue<FnType, Out, FirstArg, ArgTuple> =
    LetValue<BiClosure<FnType, Result<Out, Error>, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<FnType, Out, FirstArg, ArgTuple> ClosureLetValue<FnType, Out, FirstArg, ArgTuple>
where
    FnType: FnOnce(FirstArg, ArgTuple) -> Result<Out, Error>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new_fn_err(fn_impl: FnType) -> ClosureLetValue<FnType, Out, FirstArg, ArgTuple> {
        Self::new_err(BiClosure::new(fn_impl))
    }
}

type NoErrLetValue<FunctorType, Out, FirstArg, ArgTuple> =
    LetValue<NoErrBiFunctor<FunctorType, Out, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<FnImpl, Out, FirstArg, ArgTuple> NoErrLetValue<FnImpl, Out, FirstArg, ArgTuple>
where
    FnImpl: BiFunctor<FirstArg, ArgTuple, Output = Out>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new(fn_impl: FnImpl) -> NoErrLetValue<FnImpl, Out, FirstArg, ArgTuple> {
        Self::new_err(NoErrBiFunctor::new(fn_impl))
    }
}

type NoErrClosureLetValue<FnImpl, Out, FirstArg, ArgTuple> =
    NoErrLetValue<BiClosure<FnImpl, Out, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<FnImpl, Out, FirstArg, ArgTuple> NoErrClosureLetValue<FnImpl, Out, FirstArg, ArgTuple>
where
    FnImpl: FnOnce(FirstArg, ArgTuple) -> Out,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    pub fn new_fn(fn_impl: FnImpl) -> NoErrClosureLetValue<FnImpl, Out, FirstArg, ArgTuple> {
        Self::new(BiClosure::new(fn_impl))
    }
}

impl<FnType, Out: TypedSender, FirstArg: Scheduler, ArgTuple: IsTuple> Sender
    for LetValue<FnType, Out, FirstArg, ArgTuple>
where
    FnType: BiFunctor<FirstArg, ArgTuple, Output = Result<Out, Error>>,
{
}

impl<FnType, Out, NestedSender> BindSender<NestedSender>
    for LetValue<
        FnType,
        Out,
        <NestedSender as TypedSender>::Scheduler,
        <NestedSender as TypedSender>::Value,
    >
where
    NestedSender: TypedSender,
    FnType: BiFunctor<NestedSender::Scheduler, NestedSender::Value, Output = Result<Out, Error>>,
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
    FnType: BiFunctor<NestedSender::Scheduler, NestedSender::Value, Output = Result<Out, Error>>,
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
    FnType: BiFunctor<NestedSender::Scheduler, NestedSender::Value, Output = Result<Out, Error>>,
    Out: TypedSender,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom2: PhantomData<Out>,
}

impl<NestedSender, FnType, Out> TypedSender for LetValueSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: BiFunctor<NestedSender::Scheduler, NestedSender::Value, Output = Result<Out, Error>>,
    Out: TypedSender,
{
    type Value = Out::Value;
    type Scheduler = Out::Scheduler;

    fn connect<ReceiverImpl>(self, receiver: ReceiverImpl) -> impl OperationState
    where
        ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    {
        let wrapped_receiver = LetValueWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom0: PhantomData,
            phantom1: PhantomData,
            phantom2: PhantomData,
        };
        self.nested.connect(wrapped_receiver)
    }
}

struct LetValueWrappedReceiver<ReceiverImpl, FnType, Out, FirstArg, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: BiFunctor<FirstArg, ArgTuple, Output = Result<Out, Error>>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom0: PhantomData<FirstArg>,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<ReceiverImpl, FnType, Out, FirstArg, ArgTuple> Receiver
    for LetValueWrappedReceiver<ReceiverImpl, FnType, Out, FirstArg, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: BiFunctor<FirstArg, ArgTuple, Output = Result<Out, Error>>,
    FirstArg: Scheduler,
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

impl<ReceiverImpl, FnType, Out, FirstArg, ArgTuple> ReceiverOf<FirstArg, ArgTuple>
    for LetValueWrappedReceiver<ReceiverImpl, FnType, Out, FirstArg, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: BiFunctor<FirstArg, ArgTuple, Output = Result<Out, Error>>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn set_value(self, scheduler: FirstArg, values: ArgTuple) {
        match self.fn_impl.tuple_invoke(scheduler, values) {
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
    use crate::errors::{Error, ErrorForTesting};
    use crate::just::Just;
    use crate::just_error::JustError;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(
                Just::new((6,))
                    | LetValue::new_fn(|_, (x,)| {
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
            sync_wait(Just::new((6,)) | LetValue::new_fn_err(|_, (x,)| Ok(Just::new((x, 7, 8)))))
                .expect("should succeed")
        )
    }

    #[test]
    fn errors_from_preceding_sender_are_propagated() {
        match sync_wait(
            JustError::<()>::new(Box::new(ErrorForTesting::from("error")))
                | LetValue::new_fn(|_, ()| -> Just<(i32,)> {
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
                | LetValue::new_fn_err(|_, ()| -> Result<Just<(i32,)>, Error> {
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

    #[test]
    fn errors_from_nested_sender_are_propagated() {
        // nested_sender refers to the sender returned by the functor.
        match sync_wait(
            Just::new(())
                | LetValue::new_fn(|_, ()| {
                    JustError::<()>::new(Box::new(ErrorForTesting::from("error")))
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
