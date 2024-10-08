pub mod functor;

use crate::errors::{Error, Result};
use crate::refs::SRInto;
use crate::refs::ScopedRefMut;
use crate::scheduler::Scheduler;
use crate::scope::ScopeNest;
use crate::stop_token::StopToken;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use crate::tuple::Tuple;
use crate::tuple::TupleCat;
use functor::{BiClosure, BiFunctor, NoErrBiFunctor};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{BitOr, DerefMut};

/// Create a let-value [Sender].
///
/// A let-value sender is a sender, which, upon receiving a value, invokes a function.
/// The function returns a new [TypedSender], which will be substitued in this place of the chain.
///
/// # Function Call Arguments
///
/// Let-value takes a function which takes two arguments:
/// - a [Scheduler], which can be used to continue on the same scheduler.
/// - a [ScopedRefMut], which contains exclusive references to the value signal.
///
/// The state used by the [ScopedRefMut] argument is either [NoSendState](crate::refs::NoSendState) or [SendState](crate::refs::SendState).
/// The [ScopedRefMut] can be expanded using the [DistributeRefTuple](crate::tuple::DistributeRefTuple) trait, and can be converted to a non-mutable reference:
/// ```
/// use std::fmt::Debug;
/// use senders_receivers::refs;
/// use senders_receivers::tuple::*;
///
/// fn example<State: Clone+Debug>(reference: refs::ScopedRefMut<(i32, String), State>) {
///     // unpack the tuple
///     let (number, string) = DistributeRefTuple::distribute(reference);
///     // number: ScopedRefMut<i32, State>
///     // string: ScopedRefMut<String, State>
///
///     // change string to be a non-mutable reference.
///     let string: refs::ScopedRef<String, State> = string.into();
/// }
/// ```
///
/// # Function Call Result
///
/// The called function should return a [TypedSender].
/// The argument tuple that was passed as [ScopedRefMut], and the [value-signal](TypedSender::Value) from the returned [TypedSender], are concatenated into the output result.
///
/// The [TypedSender] can be returned directly, or it can be wrapped in a [Result].
///
/// # Examples
/// ```
/// use senders_receivers::{Just, LetValue, SyncWait};
/// use senders_receivers::refs;
///
/// // If using a function that returns a sender:
/// let sender = Just::from((String::from("world"),))
///              | LetValue::from(|_, v: refs::ScopedRefMut<(String,), refs::NoSendState>| {
///                  Just::from((format!("Hello {}!", v.0),))
///              });
/// assert_eq!(
///     (String::from("world"), String::from("Hello world!")),
///     sender.sync_wait().unwrap().unwrap());
/// ```
///
/// ```
/// use senders_receivers::{Just, LetValue, SyncWait};
/// use senders_receivers::refs;
///
/// // If using a function that returns a Result:
/// let sender = Just::from((String::from("world"),))
///              | LetValue::from(|_, v: refs::ScopedRefMut<(String,), refs::NoSendState>| {
///                  Ok(Just::from((format!("Hello {}!", v.0),)))
///              });
/// assert_eq!(
///     (String::from("world"), String::from("Hello world!")),
///     sender.sync_wait().unwrap().unwrap());
/// ```
pub struct LetValue<'a, FnType, Out, Sch, Value, State>
where
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    fn_impl: FnType,
    phantom: PhantomData<&'a fn(Sch, Value, State) -> Out>,
}

impl<'a, FnType, Out, Sch, Value, State> From<FnType>
    for LetValue<'a, FnType, Out, Sch, Value, State>
where
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    fn from(fn_impl: FnType) -> Self {
        Self {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureLetValue<'a, FnType, Out, Sch, Value, State> =
    LetValue<'a, BiClosure<'a, FnType, Result<Out>, Sch, Value, State>, Out, Sch, Value, State>;

impl<'a, FnType, Out, Sch, Value, State> From<FnType>
    for ClosureLetValue<'a, FnType, Out, Sch, Value, State>
where
    FnType: 'a + FnOnce(Sch, ScopedRefMut<Value, State>) -> Result<Out>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

type NoErrLetValue<'a, FnType, Out, Sch, Value, State> =
    LetValue<'a, NoErrBiFunctor<'a, FnType, Out, Sch, Value, State>, Out, Sch, Value, State>;

impl<'a, FnType, Out, Sch, Value, State> From<FnType>
    for NoErrLetValue<'a, FnType, Out, Sch, Value, State>
where
    FnType: BiFunctor<'a, Sch, Value, State, Output = Out>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoErrBiFunctor::new(fn_impl))
    }
}

type NoErrClosureLetValue<'a, FnType, Out, Sch, Value, State> =
    NoErrLetValue<'a, BiClosure<'a, FnType, Out, Sch, Value, State>, Out, Sch, Value, State>;

impl<'a, FnType, Out, Sch, Value, State> From<FnType>
    for NoErrClosureLetValue<'a, FnType, Out, Sch, Value, State>
where
    FnType: 'a + FnOnce(Sch, ScopedRefMut<Value, State>) -> Out,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

impl<'a, FnType, Out, Sch, Value, State> Sender for LetValue<'a, FnType, Out, Sch, Value, State>
where
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
}

impl<'a, NestedSender, FnType, Out, Sch, Value, State> BindSender<NestedSender>
    for LetValue<'a, FnType, Out, Sch, Value, State>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    type Output = LetValueTS<'a, NestedSender, FnType, Out, Sch, Value, State>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        LetValueTS {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

pub struct LetValueTS<'a, NestedSender, FnType, Out, Sch, Value, State>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<&'a fn(Sch, Value, State) -> Out>,
}

impl<'a, BindSenderImpl, NestedSender, FnType, Out, Sch, Value, State> BitOr<BindSenderImpl>
    for LetValueTS<'a, NestedSender, FnType, Out, Sch, Value, State>
where
    BindSenderImpl: BindSender<Self>,
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<'a, NestedSender, FnType, Out, Sch, Value, State> TypedSender
    for LetValueTS<'a, NestedSender, FnType, Out, Sch, Value, State>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
    State: 'static + Clone + Debug,
{
    type Value = <(Value, Out::Value) as TupleCat>::Output;
    type Scheduler = Out::Scheduler;
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverImpl, NestedSender, FnType, Out, Sch, Value, State>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverImpl>
    for LetValueTS<'a, NestedSender, FnType, Out, Sch, Value, State>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            StopTokenImpl,
            LetValueReceiver<
                'a,
                ScopeImpl,
                StopTokenImpl,
                ReceiverImpl,
                FnType,
                Out,
                Sch,
                Value,
                State,
            >,
        >,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: 'a // XXX 'a is wrong here!
        + TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is also wrong here!
            ScopeImpl::NewScopeType,
            StopTokenImpl,
            ScopeImpl::NewReceiver,
        >,
    Out::Value: 'a,
    ScopeImpl: ScopeNest<
        <Out as TypedSender>::Scheduler,
        <Out as TypedSender>::Value,
        LocalReceiverWithValue<
            <Out as TypedSender>::Scheduler,
            Value,
            <Out as TypedSender>::Value,
            ReceiverImpl,
        >,
    >,
    StopTokenImpl: StopToken,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
    ReceiverImpl: ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
    State: 'static + Clone + Debug,
    ScopedRefMut<Value, ScopeImpl::NewScopeData>: SRInto<ScopedRefMut<Value, State>>,
{
    type Output<'scope> = NestedSender::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverImpl: 'scope ;

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
        let receiver = LetValueReceiver {
            phantom: PhantomData,
            nested: receiver,
            scope: scope.clone(),
            stop_token: stop_token.clone(),
            fn_impl: self.fn_impl,
        };
        self.nested.connect(scope, stop_token, receiver)
    }
}

pub struct LetValueReceiver<
    'a,
    ScopeImpl,
    StopTokenImpl,
    NestedReceiver,
    FnType,
    Out,
    Sch,
    Value,
    State,
> where
    ScopeImpl: ScopeNest<
        <Out as TypedSender>::Scheduler,
        <Out as TypedSender>::Value,
        LocalReceiverWithValue<
            <Out as TypedSender>::Scheduler,
            Value,
            <Out as TypedSender>::Value,
            NestedReceiver,
        >,
    >,
    StopTokenImpl: StopToken,
    NestedReceiver: ReceiverOf<
        <Out as TypedSender>::Scheduler,
        <(Value, <Out as TypedSender>::Value) as TupleCat>::Output,
    >,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is wrong here
            ScopeImpl::NewScopeType,
            StopTokenImpl,
            ScopeImpl::NewReceiver,
        >,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
    State: 'static + Clone + Debug,
{
    phantom: PhantomData<&'a fn(Sch, Value, State) -> Out>,
    nested: NestedReceiver,
    scope: ScopeImpl,
    stop_token: StopTokenImpl,
    fn_impl: FnType,
}

impl<'a, ScopeImpl, StopTokenImpl, NestedReceiver, FnType, Out, Sch, Value, State> Receiver
    for LetValueReceiver<
        'a,
        ScopeImpl,
        StopTokenImpl,
        NestedReceiver,
        FnType,
        Out,
        Sch,
        Value,
        State,
    >
where
    ScopeImpl: ScopeNest<
        <Out as TypedSender>::Scheduler,
        <Out as TypedSender>::Value,
        LocalReceiverWithValue<
            <Out as TypedSender>::Scheduler,
            Value,
            <Out as TypedSender>::Value,
            NestedReceiver,
        >,
    >,
    StopTokenImpl: StopToken,
    NestedReceiver: ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is wrong here
            ScopeImpl::NewScopeType,
            StopTokenImpl,
            ScopeImpl::NewReceiver,
        >,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
    State: 'static + Clone + Debug,
{
    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }

    fn set_done(self) {
        self.nested.set_done();
    }
}

impl<'a, ScopeImpl, StopTokenImpl, NestedReceiver, FnType, Out, Sch, Value, State>
    ReceiverOf<Sch, Value>
    for LetValueReceiver<
        'a,
        ScopeImpl,
        StopTokenImpl,
        NestedReceiver,
        FnType,
        Out,
        Sch,
        Value,
        State,
    >
where
    ScopeImpl: ScopeNest<
        <Out as TypedSender>::Scheduler,
        <Out as TypedSender>::Value,
        LocalReceiverWithValue<
            <Out as TypedSender>::Scheduler,
            Value,
            <Out as TypedSender>::Value,
            NestedReceiver,
        >,
    >,
    StopTokenImpl: StopToken,
    NestedReceiver:
        ReceiverOf<Out::Scheduler, <(Value, <Out as TypedSender>::Value) as TupleCat>::Output>,
    FnType: 'a + BiFunctor<'a, Sch, Value, State, Output = Result<Out>>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'a + Tuple,
    Out: TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is wrong here
            ScopeImpl::NewScopeType,
            StopTokenImpl,
            ScopeImpl::NewReceiver,
        >,
    Out::Value: 'a,
    (Value, <Out as TypedSender>::Value): TupleCat,
    <(Value, <Out as TypedSender>::Value) as TupleCat>::Output: 'a + Tuple,
    State: 'static + Clone + Debug,
    ScopedRefMut<Value, ScopeImpl::NewScopeData>: SRInto<ScopedRefMut<Value, State>>,
{
    fn set_value(self, sch: Sch, value: Value) {
        let local_receiver_with_value = LocalReceiverWithValue::new(value, self.nested);
        let (local_scope, local_receiver, rcv_ref) =
            self.scope.new_scope(local_receiver_with_value);

        let values_ref = ScopedRefMut::map(
            rcv_ref,
            |rcv: &mut LocalReceiverWithValue<
                <Out as TypedSender>::Scheduler,
                Value,
                <Out as TypedSender>::Value,
                NestedReceiver,
            >| rcv.values_ref(),
        );

        match self.fn_impl.tuple_invoke(sch, values_ref.sr_into()) {
            Ok(local_sender) => local_sender
                .connect(&local_scope, self.stop_token, local_receiver)
                .start(),
            Err(error) => local_receiver.set_error(error),
        };
    }
}

pub struct LocalReceiverWithValue<Sch, Value, OutValue, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    OutValue: Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: Tuple,
    Rcv: ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
{
    phantom: PhantomData<fn(Sch, OutValue)>,
    value: Box<Value>,
    rcv: Rcv,
}

impl<Sch, Value, OutValue, Rcv> LocalReceiverWithValue<Sch, Value, OutValue, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    OutValue: Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: Tuple,
    Rcv: ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
{
    fn new(value: Value, rcv: Rcv) -> Self {
        Self {
            phantom: PhantomData,
            value: Box::new(value),
            rcv,
        }
    }

    fn values_ref(&mut self) -> &mut Value {
        self.value.deref_mut()
    }
}

impl<Sch, Value, OutValue, Rcv> Receiver for LocalReceiverWithValue<Sch, Value, OutValue, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    OutValue: Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: Tuple,
    Rcv: ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
{
    fn set_error(self, error: Error) {
        self.rcv.set_error(error);
    }

    fn set_done(self) {
        self.rcv.set_done();
    }
}

impl<Sch, Value, OutValue, Rcv> ReceiverOf<Sch, OutValue>
    for LocalReceiverWithValue<Sch, Value, OutValue, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    OutValue: Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: Tuple,
    Rcv: ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
{
    fn set_value(self, sch: Sch, value: OutValue) {
        self.rcv.set_value(sch, (*self.value, value).cat());
    }
}

#[cfg(test)]
mod tests {
    use super::LetValue;
    use crate::errors::{new_error, ErrorForTesting, Result};
    use crate::just::Just;
    use crate::just_error::JustError;
    use crate::refs;
    use crate::scheduler::{ImmediateScheduler, Scheduler};
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((6, 7, 8)),
            (Just::from((6,))
                | LetValue::from(|_, x: refs::ScopedRefMut<(i32,), refs::NoSendState>| {
                    assert_eq!(x.0, 6);
                    Just::from((7, 8))
                }))
            .sync_wait()
            .expect("should succeed")
        )
    }

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            Some((6, 7, 8)),
            (Just::from((6,))
                | LetValue::from(|_, _: refs::ScopedRefMut<(i32,), refs::NoSendState>| Ok(
                    Just::from((7, 8))
                )))
            .sync_wait()
            .expect("should succeed")
        )
    }

    #[test]
    fn errors_from_preceding_sender_are_propagated() {
        match (JustError::<ImmediateScheduler, ()>::from(new_error(ErrorForTesting::from("error")))
            | LetValue::from(
                |_,
                 _: refs::ScopedRefMut<(), refs::NoSendState>|
                 -> Just<ImmediateScheduler, (i32,)> {
                    panic!("expect this function to not be invoked")
                },
            ))
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
            | LetValue::from(
                |_,
                 _: refs::ScopedRefMut<(), refs::NoSendState>|
                 -> Result<Just<ImmediateScheduler, (i32,)>> {
                    Err(new_error(ErrorForTesting::from("error")))
                },
            ))
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
    fn errors_from_nested_sender_are_propagated() {
        // nested_sender refers to the sender returned by the functor.
        match (Just::from(())
            | LetValue::from(
                |sch: ImmediateScheduler, _: refs::ScopedRefMut<(), refs::NoSendState>| {
                    sch.schedule_error::<()>(new_error(ErrorForTesting::from("error")))
                },
            ))
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
