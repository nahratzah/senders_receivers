use crate::errors::{Error, Result};
use crate::functor::{NoArgClosure, NoArgFunctor, NoErrNoArgFunctor};
use crate::stop_token::StopToken;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use std::marker::PhantomData;
use std::ops::BitOr;

/// Create a let-done [Sender].
///
/// A let-done sender is a sender, which, upon receiving the done-signal, invokes a function.
/// The function returns a new [TypedSender], which will be substituted in this place of the chain.
/// The returned sender must use matching [Scheduler](crate::scheduler::Scheduler) and [TypedSender::Value] type.
///
/// Example:
/// ```
/// use senders_receivers::{ImmediateScheduler, LetDone, Scheduler, Then, new_error, SyncWait};
/// use std::io;
///
/// let sender = ImmediateScheduler::default().schedule_done::<(String,)>()
///              | LetDone::from(|| {
///                  ImmediateScheduler::default().schedule_value((String::from("hello"), String::from("world")))
///                  | Then::from(|(greeting, who)| (format!("{}, {}!", greeting, who),))
///              });
/// println!("{}", sender.sync_wait().unwrap().unwrap().0);
/// ```
pub struct LetDone<'a, FnType, Out>
where
    FnType: 'a + NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender,
{
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
}

impl<'a, FnType, Out> From<FnType> for LetDone<'a, FnType, Out>
where
    FnType: 'a + NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        LetDone {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureLetDone<'a, FnType, Out> = LetDone<'a, NoArgClosure<'a, FnType, Result<Out>>, Out>;

impl<'a, FnType, Out> From<FnType> for ClosureLetDone<'a, FnType, Out>
where
    FnType: 'a + FnOnce() -> Result<Out>,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoArgClosure::new(fn_impl))
    }
}

type NoErrLetDone<'a, FnType, Out> = LetDone<'a, NoErrNoArgFunctor<'a, FnType, Out>, Out>;

impl<'a, FnType, Out> From<FnType> for NoErrLetDone<'a, FnType, Out>
where
    FnType: 'a + NoArgFunctor<'a, Output = Out>,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoErrNoArgFunctor::new(fn_impl))
    }
}

type NoErrClosureLetDone<'a, FnType, Out> = NoErrLetDone<'a, NoArgClosure<'a, FnType, Out>, Out>;

impl<'a, FnType, Out> From<FnType> for NoErrClosureLetDone<'a, FnType, Out>
where
    FnType: 'a + FnOnce() -> Out,
    Out: TypedSender,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoArgClosure::new(fn_impl))
    }
}

impl<'a, FnType, Out> Sender for LetDone<'a, FnType, Out>
where
    FnType: 'a + NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender,
{
}

impl<'a, FnType, Out, NestedSender> BindSender<NestedSender> for LetDone<'a, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: 'a + NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender,
{
    type Output = LetDoneSender<'a, NestedSender, FnType, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        LetDoneSender {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

pub struct LetDoneSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: 'a + NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
}

impl<'a, FnType, Out, NestedSender> TypedSender for LetDoneSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: 'a + NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender,
{
    type Value = Out::Value;
    type Scheduler = Out::Scheduler;
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType, FnType, Out, NestedSender>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType>
    for LetDoneSender<'a, NestedSender, FnType, Out>
where
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            StopTokenImpl,
            ReceiverWrapper<'a, ScopeImpl, StopTokenImpl, ReceiverType, FnType, Out>,
        >,
    FnType: 'a + NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender + TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType>,
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    ScopeImpl: Clone,
    StopTokenImpl: StopToken,
{
    type Output<'scope> = NestedSender::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        stop_token: StopTokenImpl,
        receiver: ReceiverType,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope,
    {
        let receiver = ReceiverWrapper {
            receiver,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
            scope: scope.clone(),
            stop_token: stop_token.clone(),
        };
        self.nested.connect(scope, stop_token, receiver)
    }
}

impl<'a, BindSenderImpl, NestedSender, FnType, Out> BitOr<BindSenderImpl>
    for LetDoneSender<'a, NestedSender, FnType, Out>
where
    BindSenderImpl: BindSender<Self>,
    NestedSender: TypedSender<Scheduler = Out::Scheduler, Value = Out::Value>,
    FnType: NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSender,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ReceiverWrapper<'a, ScopeImpl, StopTokenImpl, ReceiverType, FnType, Out>
where
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType>,
    StopTokenImpl: StopToken,
{
    receiver: ReceiverType,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn() -> Out>,
    scope: ScopeImpl,
    stop_token: StopTokenImpl,
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType, FnType, Out> Receiver
    for ReceiverWrapper<'a, ScopeImpl, StopTokenImpl, ReceiverType, FnType, Out>
where
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType>,
    StopTokenImpl: StopToken,
{
    fn set_done(self) {
        match self.fn_impl.tuple_invoke() {
            Ok(sender) => sender
                .connect(&self.scope, self.stop_token, self.receiver)
                .start(),
            Err(error) => self.receiver.set_error(error),
        };
    }

    fn set_error(self, error: Error) {
        self.receiver.set_error(error);
    }
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType, FnType, Out> ReceiverOf<Out::Scheduler, Out::Value>
    for ReceiverWrapper<'a, ScopeImpl, StopTokenImpl, ReceiverType, FnType, Out>
where
    ReceiverType: ReceiverOf<Out::Scheduler, Out::Value>,
    FnType: NoArgFunctor<'a, Output = Result<Out>>,
    Out: TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType>,
    StopTokenImpl: StopToken,
{
    fn set_value(self, sch: Out::Scheduler, value: Out::Value) {
        self.receiver.set_value(sch, value);
    }
}

#[cfg(test)]
mod tests {
    use super::LetDone;
    use crate::errors::new_error;
    use crate::errors::ErrorForTesting;
    use crate::errors::Result;
    use crate::just::Just;
    use crate::scheduler::{ImmediateScheduler, Scheduler};
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_works() {
        assert_eq!(
            (String::from("yay"),),
            (ImmediateScheduler.schedule_done::<(String,)>()
                | LetDone::from(|| { ImmediateScheduler.schedule_value((String::from("yay"),)) }))
            .sync_wait()
            .expect("should succeed")
            .expect("should not be the done signal")
        );
    }

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            ErrorForTesting::from("this error will be returned"),
            *(ImmediateScheduler.schedule_done::<(String,)>()
                | LetDone::from(|| {
                    let result: Result<Just<ImmediateScheduler, (String,)>> = Err(new_error(
                        ErrorForTesting::from("this error will be returned"),
                    ));
                    result
                }))
            .sync_wait()
            .expect_err("should return an error")
            .downcast_ref::<ErrorForTesting>()
            .unwrap()
        );
    }

    #[test]
    fn it_cascades_error() {
        assert_eq!(
            ErrorForTesting::from("should be passed through"),
            *(ImmediateScheduler.schedule_error::<(String,)>(new_error(ErrorForTesting::from(
                "should be passed through"
            ))) | LetDone::from(|| -> Just<ImmediateScheduler, (String,)> {
                panic!("should not be called!");
            }))
            .sync_wait()
            .expect_err("should return the error")
            .downcast_ref::<ErrorForTesting>()
            .unwrap()
        );
    }

    #[test]
    fn it_cascades_value() {
        assert_eq!(
            Some((String::from("yay"),)),
            (ImmediateScheduler.schedule_value((String::from("yay"),))
                | LetDone::from(|| -> Just<ImmediateScheduler, (String,)> {
                    panic!("should not be called!");
                }))
            .sync_wait()
            .expect("should succeed")
        );
    }
}
