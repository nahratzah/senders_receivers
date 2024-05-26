use crate::errors::{Error, IsTuple};
use crate::functor::{BiClosure, BiFunctor, NoErrBiFunctor};
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use std::marker::PhantomData;
use std::ops::BitOr;

/// Create a let-value [Sender].
///
/// A let-value sender is a sender, which, upon receiving a value, invokes a function.
/// The function returns a new [TypedSender], which will be substitued in this place of the chain.
///
/// Example:
/// ```
/// use senders_receivers::{Just, LetValue, sync_wait};
///
/// // If using a function that returns a sender:
/// let sender = Just::from((String::from("world"),))
///              | LetValue::from(|_, (name,)| {
///                  Just::from((format!("Hello {}!", name),))
///              });
/// assert_eq!(
///     (String::from("Hello world!"),),
///     sync_wait(sender).unwrap().unwrap());
///
/// // If using a function that returns a Result:
/// let sender = Just::from((String::from("world"),))
///              | LetValue::from(|_, (name,)| {
///                  Ok(Just::from((format!("Hello {}!", name),)))
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
    phantom: PhantomData<fn(FirstArg, ArgTuple) -> Out>,
}

impl<FnType, Out, FirstArg, ArgTuple> From<FnType> for LetValue<FnType, Out, FirstArg, ArgTuple>
where
    FnType: BiFunctor<FirstArg, ArgTuple, Output = Result<Out, Error>>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        LetValue {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureLetValue<FnType, Out, FirstArg, ArgTuple> =
    LetValue<BiClosure<FnType, Result<Out, Error>, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<FnType, Out, FirstArg, ArgTuple> From<FnType>
    for ClosureLetValue<FnType, Out, FirstArg, ArgTuple>
where
    FnType: FnOnce(FirstArg, ArgTuple) -> Result<Out, Error>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

type NoErrLetValue<FunctorType, Out, FirstArg, ArgTuple> =
    LetValue<NoErrBiFunctor<FunctorType, Out, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<FnImpl, Out, FirstArg, ArgTuple> From<FnImpl>
    for NoErrLetValue<FnImpl, Out, FirstArg, ArgTuple>
where
    FnImpl: BiFunctor<FirstArg, ArgTuple, Output = Out>,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(NoErrBiFunctor::new(fn_impl))
    }
}

type NoErrClosureLetValue<FnImpl, Out, FirstArg, ArgTuple> =
    NoErrLetValue<BiClosure<FnImpl, Out, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<FnImpl, Out, FirstArg, ArgTuple> From<FnImpl>
    for NoErrClosureLetValue<FnImpl, Out, FirstArg, ArgTuple>
where
    FnImpl: FnOnce(FirstArg, ArgTuple) -> Out,
    FirstArg: Scheduler,
    ArgTuple: IsTuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(BiClosure::new(fn_impl))
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
            phantom: PhantomData,
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
    phantom: PhantomData<fn() -> Out>,
}

impl<NestedSender, FnType, Out> TypedSender for LetValueSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: BiFunctor<NestedSender::Scheduler, NestedSender::Value, Output = Result<Out, Error>>,
    Out: TypedSender,
{
    type Value = Out::Value;
    type Scheduler = Out::Scheduler;
}

impl<ReceiverImpl, NestedSender, FnType, Out> TypedSenderConnect<ReceiverImpl>
    for LetValueSender<NestedSender, FnType, Out>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    NestedSender: TypedSender
        + TypedSenderConnect<
            LetValueWrappedReceiver<
                ReceiverImpl,
                FnType,
                Out,
                <NestedSender as TypedSender>::Scheduler,
                <NestedSender as TypedSender>::Value,
            >,
        >,
    FnType: BiFunctor<NestedSender::Scheduler, NestedSender::Value, Output = Result<Out, Error>>,
    Out: TypedSender + TypedSenderConnect<ReceiverImpl>,
{
    fn connect(self, receiver: ReceiverImpl) -> impl OperationState {
        let wrapped_receiver = LetValueWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
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
    phantom: PhantomData<fn(FirstArg, ArgTuple) -> Out>,
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
    Out: TypedSender + TypedSenderConnect<ReceiverImpl>,
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
    use crate::errors::{new_error, Error, ErrorForTesting};
    use crate::just::Just;
    use crate::just_error::JustError;
    use crate::scheduler::{ImmediateScheduler, WithScheduler};
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(
                Just::from((6,))
                    | LetValue::from(|_, (x,)| {
                        assert_eq!(x, 6);
                        Just::from((x, 7, 8))
                    })
            )
            .expect("should succeed")
        )
    }

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(Just::from((6,)) | LetValue::from(|_, (x,)| Ok(Just::from((x, 7, 8)))))
                .expect("should succeed")
        )
    }

    #[test]
    fn errors_from_preceding_sender_are_propagated() {
        match sync_wait(
            JustError::<ImmediateScheduler, ()>::from(new_error(ErrorForTesting::from("error")))
                | LetValue::from(|_, ()| -> Just<ImmediateScheduler, (i32,)> {
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
            Just::from(())
                | LetValue::from(|_, ()| -> Result<Just<ImmediateScheduler, (i32,)>, Error> {
                    Err(new_error(ErrorForTesting::from("error")))
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
            Just::from(())
                | LetValue::from(|sch: ImmediateScheduler, ()| {
                    JustError::<ImmediateScheduler, ()>::with_scheduler(
                        sch,
                        new_error(ErrorForTesting::from("error")),
                    )
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
