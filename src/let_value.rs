use crate::errors::{Error, Result, Tuple};
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
pub struct LetValue<'a, FnType, Out, FirstArg, ArgTuple>
where
    FnType: BiFunctor<'a, FirstArg, ArgTuple, Output = Result<Out>>,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
    Out: TypedSender,
{
    fn_impl: FnType,
    phantom: PhantomData<&'a fn(FirstArg, ArgTuple) -> Out>,
}

impl<'a, FnType, Out, FirstArg, ArgTuple> From<FnType>
    for LetValue<'a, FnType, Out, FirstArg, ArgTuple>
where
    FnType: BiFunctor<'a, FirstArg, ArgTuple, Output = Result<Out>>,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        LetValue {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureLetValue<'a, FnType, Out, FirstArg, ArgTuple> =
    LetValue<'a, BiClosure<'a, FnType, Result<Out>, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<'a, FnType, Out, FirstArg, ArgTuple> From<FnType>
    for ClosureLetValue<'a, FnType, Out, FirstArg, ArgTuple>
where
    FnType: 'a + FnOnce(FirstArg, ArgTuple) -> Result<Out>,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

type NoErrLetValue<'a, FunctorType, Out, FirstArg, ArgTuple> =
    LetValue<'a, NoErrBiFunctor<'a, FunctorType, Out, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<'a, FnImpl, Out, FirstArg, ArgTuple> From<FnImpl>
    for NoErrLetValue<'a, FnImpl, Out, FirstArg, ArgTuple>
where
    FnImpl: BiFunctor<'a, FirstArg, ArgTuple, Output = Out>,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(NoErrBiFunctor::new(fn_impl))
    }
}

type NoErrClosureLetValue<'a, FnImpl, Out, FirstArg, ArgTuple> =
    NoErrLetValue<'a, BiClosure<'a, FnImpl, Out, FirstArg, ArgTuple>, Out, FirstArg, ArgTuple>;

impl<'a, FnImpl, Out, FirstArg, ArgTuple> From<FnImpl>
    for NoErrClosureLetValue<'a, FnImpl, Out, FirstArg, ArgTuple>
where
    FnImpl: 'a + FnOnce(FirstArg, ArgTuple) -> Out,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
    Out: TypedSender,
{
    fn from(fn_impl: FnImpl) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

impl<'a, FnType, Out: TypedSender, FirstArg: Scheduler, ArgTuple: Tuple> Sender
    for LetValue<'a, FnType, Out, FirstArg, ArgTuple>
where
    FnType: BiFunctor<'a, FirstArg, ArgTuple, Output = Result<Out>>,
{
}

impl<'a, FnType, Out, NestedSender> BindSender<NestedSender>
    for LetValue<
        'a,
        FnType,
        Out,
        <NestedSender as TypedSender>::Scheduler,
        <NestedSender as TypedSender>::Value,
    >
where
    NestedSender: TypedSender,
    FnType: BiFunctor<'a, NestedSender::Scheduler, NestedSender::Value, Output = Result<Out>>,
    NestedSender::Value: Tuple,
    Out: TypedSender,
{
    type Output = LetValueSender<'a, NestedSender, FnType, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        LetValueSender {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

impl<'a, NestedSender, FnType, Out: TypedSender, BindSenderImpl> BitOr<BindSenderImpl>
    for LetValueSender<'a, NestedSender, FnType, Out>
where
    BindSenderImpl: BindSender<Self>,
    NestedSender: TypedSender,
    FnType: BiFunctor<'a, NestedSender::Scheduler, NestedSender::Value, Output = Result<Out>>,
    NestedSender::Value: Tuple,
    Out: TypedSender,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct LetValueSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: BiFunctor<'a, NestedSender::Scheduler, NestedSender::Value, Output = Result<Out>>,
    Out: TypedSender,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
}

impl<'a, NestedSender, FnType, Out> TypedSender for LetValueSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: BiFunctor<'a, NestedSender::Scheduler, NestedSender::Value, Output = Result<Out>>,
    Out: TypedSender,
{
    type Value = Out::Value;
    type Scheduler = Out::Scheduler;
}

impl<'a, ReceiverImpl, NestedSender, FnType, Out> TypedSenderConnect<ReceiverImpl>
    for LetValueSender<'a, NestedSender, FnType, Out>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    NestedSender: TypedSender
        + TypedSenderConnect<
            LetValueWrappedReceiver<
                'a,
                ReceiverImpl,
                FnType,
                Out,
                <NestedSender as TypedSender>::Scheduler,
                <NestedSender as TypedSender>::Value,
            >,
        >,
    NestedSender::Value: 'a,
    FnType: BiFunctor<'a, NestedSender::Scheduler, NestedSender::Value, Output = Result<Out>>,
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

struct LetValueWrappedReceiver<'a, ReceiverImpl, FnType, Out, FirstArg, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: BiFunctor<'a, FirstArg, ArgTuple, Output = Result<Out>>,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
    Out: TypedSender + TypedSenderConnect<ReceiverImpl>,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn(FirstArg, ArgTuple) -> Out>,
}

impl<'a, ReceiverImpl, FnType, Out, FirstArg, ArgTuple> Receiver
    for LetValueWrappedReceiver<'a, ReceiverImpl, FnType, Out, FirstArg, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: BiFunctor<'a, FirstArg, ArgTuple, Output = Result<Out>>,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
    Out: TypedSender + TypedSenderConnect<ReceiverImpl>,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<'a, ReceiverImpl, FnType, Out, FirstArg, ArgTuple> ReceiverOf<FirstArg, ArgTuple>
    for LetValueWrappedReceiver<'a, ReceiverImpl, FnType, Out, FirstArg, ArgTuple>
where
    ReceiverImpl: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: BiFunctor<'a, FirstArg, ArgTuple, Output = Result<Out>>,
    FirstArg: Scheduler,
    ArgTuple: Tuple,
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
    use crate::errors::{new_error, ErrorForTesting, Result};
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
                | LetValue::from(|_, ()| -> Result<Just<ImmediateScheduler, (i32,)>> {
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
