use crate::errors::{Error, Result};
use crate::functor::{Closure, Functor, NoErrFunctor};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use std::marker::PhantomData;
use std::ops::BitOr;

/// Create a let-error [Sender].
///
/// A let-error sender is a sender, which, upon receiving an error, invokes a function.
/// The function returns a new [TypedSender], which will be substituted in this place of the chain.
/// The returned sender must use matching [Scheduler](crate::scheduler::Scheduler) and [TypedSender::Value] type.
///
/// Example:
/// ```
/// use senders_receivers::{ImmediateScheduler, LetError, Scheduler, Then, new_error, SyncWait};
/// use std::io;
///
/// let sender = ImmediateScheduler::default().schedule_error::<(String,)>(new_error(io::Error::new(io::ErrorKind::Other, "oh no!")))
///              | LetError::from(|error| {
///                  ImmediateScheduler::default().schedule_value((String::from("hello"), String::from("world")))
///                  | Then::from(|(greeting, who)| (format!("{}, {}!", greeting, who),))
///              });
/// println!("{}", sender.sync_wait().unwrap().unwrap().0);
/// ```
pub struct LetError<'a, FnType, Out>
where
    FnType: 'a + Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender,
{
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
}

impl<'a, FnType, Out> From<FnType> for LetError<'a, FnType, Out>
where
    FnType: 'a + Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        LetError {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureLetError<'a, FnType, Out> = LetError<'a, Closure<'a, FnType, Result<Out>, Error>, Out>;

impl<'a, FnType, Out> From<FnType> for ClosureLetError<'a, FnType, Out>
where
    FnType: 'a + FnOnce(Error) -> Result<Out>,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(Closure::new(fn_impl))
    }
}

type NoErrLetError<'a, FnType, Out> = LetError<'a, NoErrFunctor<'a, FnType, Out, Error>, Out>;

impl<'a, FnType, Out> From<FnType> for NoErrLetError<'a, FnType, Out>
where
    FnType: 'a + Functor<'a, Error, Output = Out>,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoErrFunctor::new(fn_impl))
    }
}

type NoErrClosureLetError<'a, FnType, Out> =
    NoErrLetError<'a, Closure<'a, FnType, Out, Error>, Out>;

impl<'a, FnType, Out> From<FnType> for NoErrClosureLetError<'a, FnType, Out>
where
    FnType: 'a + FnOnce(Error) -> Out,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(Closure::new(fn_impl))
    }
}

impl<'a, FnType, Out> Sender for LetError<'a, FnType, Out>
where
    FnType: 'a + Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender,
{
}

impl<'a, FnType, Out, NestedSender> BindSender<NestedSender> for LetError<'a, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: 'a + Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender,
{
    type Output = LetErrorSender<'a, NestedSender, FnType, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        LetErrorSender {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

pub struct LetErrorSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: 'a + Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
}

impl<'a, FnType, Out, NestedSender> TypedSender for LetErrorSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: 'a + Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender,
{
    type Value = Out::Value;
    type Scheduler = Out::Scheduler;
}

impl<'a, ScopeImpl, ReceiverType, FnType, Out, NestedSender>
    TypedSenderConnect<'a, ScopeImpl, ReceiverType>
    for LetErrorSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>
        + TypedSenderConnect<'a, ScopeImpl, ReceiverWrapper<'a, ScopeImpl, ReceiverType, FnType, Out>>,
    FnType: 'a + Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender + TypedSenderConnect<'a, ScopeImpl, ReceiverType>,
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    ScopeImpl: Clone,
{
    type Output<'scope> = NestedSender::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(self, scope: &ScopeImpl, receiver: ReceiverType) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        let receiver = ReceiverWrapper {
            receiver,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
            scope: scope.clone(),
        };
        self.nested.connect(scope, receiver)
    }
}

impl<'a, BindSenderImpl, NestedSender, FnType, Out> BitOr<BindSenderImpl>
    for LetErrorSender<'a, NestedSender, FnType, Out>
where
    BindSenderImpl: BindSender<Self>,
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSender,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ReceiverWrapper<'a, ScopeImpl, ReceiverType, FnType, Out>
where
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSenderConnect<'a, ScopeImpl, ReceiverType>,
{
    receiver: ReceiverType,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
    scope: ScopeImpl,
}

impl<'a, ScopeImpl, ReceiverType, FnType, Out> Receiver
    for ReceiverWrapper<'a, ScopeImpl, ReceiverType, FnType, Out>
where
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSenderConnect<'a, ScopeImpl, ReceiverType>,
{
    fn set_done(self) {
        self.receiver.set_done();
    }

    fn set_error(self, error: Error) {
        match self.fn_impl.tuple_invoke(error) {
            Ok(sender) => sender.connect(&self.scope, self.receiver).start(),
            Err(error) => self.receiver.set_error(error),
        };
    }
}

impl<'a, ScopeImpl, ReceiverType, FnType, Out> ReceiverOf<Out::Scheduler, Out::Value>
    for ReceiverWrapper<'a, ScopeImpl, ReceiverType, FnType, Out>
where
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: Functor<'a, Error, Output = Result<Out>>,
    Out: TypedSenderConnect<'a, ScopeImpl, ReceiverType>,
{
    fn set_value(self, sch: Out::Scheduler, value: Out::Value) {
        self.receiver.set_value(sch, value);
    }
}

#[cfg(test)]
mod tests {
    use super::LetError;
    use crate::errors::{new_error, Error, ErrorForTesting};
    use crate::just::Just;
    use crate::scheduler::{ImmediateScheduler, Scheduler};
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((String::from("yay"),)),
            (ImmediateScheduler.schedule_error::<(String,)>(new_error(
                ErrorForTesting::from("this error will be consumed")
            )) | LetError::from(|error: Error| {
                assert_eq!(
                    ErrorForTesting::from("this error will be consumed"),
                    *error.downcast_ref::<ErrorForTesting>().unwrap()
                );
                ImmediateScheduler.schedule_value((String::from("yay"),))
            }))
            .sync_wait()
            .expect("should succeed")
        );
    }

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            ErrorForTesting::from("this error will be returned"),
            *(ImmediateScheduler.schedule_error::<(String,)>(new_error(
                ErrorForTesting::from("this error will be consumed")
            )) | LetError::from(|error: Error| {
                match error.downcast_ref::<ErrorForTesting>() {
                    Some(_) => Err(new_error(ErrorForTesting::from(
                        "this error will be returned",
                    ))),
                    None => {
                        Ok(ImmediateScheduler.schedule_value((String::from("nay"),)))
                    }
                }
            }))
            .sync_wait()
            .expect_err("should return an error")
            .downcast_ref::<ErrorForTesting>()
            .unwrap()
        );
    }

    #[test]
    fn it_cascades_done() {
        assert_eq!(
            None,
            (ImmediateScheduler.schedule_done::<(String,)>()
                | LetError::from(|_: Error| -> Just<ImmediateScheduler, (String,)> {
                    panic!("should not be called!");
                }))
            .sync_wait()
            .expect("should succeed")
        );
    }

    #[test]
    fn it_cascades_value() {
        assert_eq!(
            Some((String::from("yay"),)),
            (ImmediateScheduler.schedule_value((String::from("yay"),))
                | LetError::from(|_: Error| -> Just<ImmediateScheduler, (String,)> {
                    panic!("should not be called!");
                }))
            .sync_wait()
            .expect("should succeed")
        );
    }
}
