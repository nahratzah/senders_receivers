pub mod functor;

use crate::errors::{Error, Result};
use crate::refs::ScopedRefMut;
use crate::scheduler::Scheduler;
use crate::scope::ScopeNest;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use crate::tuple::Tuple;
use crate::tuple::TupleCat;
use functor::{BiClosure, BiFunctor, NoErrBiFunctor};
use std::marker::PhantomData;
use std::mem;
use std::ops::{BitOr, DerefMut};

/// Create a let-value [Sender].
///
/// A let-value sender is a sender, which, upon receiving a value, invokes a function.
/// The function returns a new [TypedSender], which will be substitued in this place of the chain.
///
/// Example:
/// ```
/// use senders_receivers::{Just, LetValue, SyncWait};
///
/// // If using a function that returns a sender:
/// let sender = Just::from((String::from("world"),))
///              | LetValue::from(|_, v: &mut (String,)| {
///                  Just::from((format!("Hello {}!", v.0),))
///              });
/// assert_eq!(
///     (String::from("world"), String::from("Hello world!")),
///     sender.sync_wait().unwrap().unwrap());
///
/// // If using a function that returns a Result:
/// let sender = Just::from((String::from("world"),))
///              | LetValue::from(|_, v: &mut (String,)| {
///                  Ok(Just::from((format!("Hello {}!", v.0),)))
///              });
/// assert_eq!(
///     (String::from("world"), String::from("Hello world!")),
///     sender.sync_wait().unwrap().unwrap());
/// ```
pub struct LetValue<'a, FnType, Out, Sch, Value>
where
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn_impl: FnType,
    phantom: PhantomData<(&'a (), fn(Sch, Value) -> Out)>,
}

impl<'a, FnType, Out, Sch, Value> From<FnType> for LetValue<'a, FnType, Out, Sch, Value>
where
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

type ClosureLetValue<'a, FnType, Out, Sch, Value> =
    LetValue<'a, BiClosure<'a, FnType, Result<Out>, Sch, Value>, Out, Sch, Value>;

impl<'a, FnType, Out, Sch, Value> From<FnType> for ClosureLetValue<'a, FnType, Out, Sch, Value>
where
    FnType: 'a + for<'scope> FnOnce(Sch, &'scope mut Value) -> Result<Out>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

type NoErrLetValue<'a, FnType, Out, Sch, Value> =
    LetValue<'a, NoErrBiFunctor<'a, FnType, Out, Sch, Value>, Out, Sch, Value>;

impl<'a, FnType, Out, Sch, Value> From<FnType> for NoErrLetValue<'a, FnType, Out, Sch, Value>
where
    FnType: BiFunctor<'a, Sch, Value, Output = Out>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoErrBiFunctor::new(fn_impl))
    }
}

type NoErrClosureLetValue<'a, FnType, Out, Sch, Value> =
    NoErrLetValue<'a, BiClosure<'a, FnType, Out, Sch, Value>, Out, Sch, Value>;

impl<'a, FnType, Out, Sch, Value> From<FnType> for NoErrClosureLetValue<'a, FnType, Out, Sch, Value>
where
    FnType: 'a + for<'scope> FnOnce(Sch, &'scope mut Value) -> Out,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

impl<'a, FnType, Out, Sch, Value> Sender for LetValue<'a, FnType, Out, Sch, Value>
where
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
}

impl<'a, NestedSender, FnType, Out, Sch, Value> BindSender<NestedSender>
    for LetValue<'a, FnType, Out, Sch, Value>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    type Output = LetValueTS<'a, NestedSender, FnType, Out, Sch, Value>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        LetValueTS {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

pub struct LetValueTS<'a, NestedSender, FnType, Out, Sch, Value>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<(&'a (), fn(Sch, Value) -> Out)>,
}

impl<'a, BindSenderImpl, NestedSender, FnType, Out, Sch, Value> BitOr<BindSenderImpl>
    for LetValueTS<'a, NestedSender, FnType, Out, Sch, Value>
where
    BindSenderImpl: BindSender<Self>,
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<'a, NestedSender, FnType, Out, Sch, Value> TypedSender
    for LetValueTS<'a, NestedSender, FnType, Out, Sch, Value>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    type Value = <(Value, Out::Value) as TupleCat>::Output;
    type Scheduler = Out::Scheduler;
}

impl<'a, ScopeImpl, ReceiverImpl, NestedSender, FnType, Out, Sch, Value>
    TypedSenderConnect<'a, ScopeImpl, ReceiverImpl>
    for LetValueTS<'a, NestedSender, FnType, Out, Sch, Value>
where
    NestedSender: TypedSender<Scheduler = Sch, Value = Value>
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            LetValueReceiver<'a, ScopeImpl, ReceiverImpl, FnType, Out, Sch, Value>,
        >,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: 'a // XXX 'a is wrong here!
        + TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is also wrong here!
            ScopeImpl::NewScopeType,
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
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
    ReceiverImpl: ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
{
    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        receiver: ReceiverImpl,
    ) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverImpl: 'scope,
    {
        let receiver = LetValueReceiver {
            phantom: PhantomData,
            nested: receiver,
            scope: scope.clone(),
            fn_impl: self.fn_impl,
        };
        self.nested.connect(scope, receiver)
    }
}

struct LetValueReceiver<'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value>
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
    NestedReceiver: ReceiverOf<
        <Out as TypedSender>::Scheduler,
        <(Value, <Out as TypedSender>::Value) as TupleCat>::Output,
    >,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is wrong here
            ScopeImpl::NewScopeType,
            ScopeImpl::NewReceiver,
        >,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
{
    phantom: PhantomData<(&'a (), fn(Sch, Value) -> Out)>,
    nested: NestedReceiver,
    scope: ScopeImpl,
    fn_impl: FnType,
}

impl<'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value> Receiver
    for LetValueReceiver<'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value>
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
    NestedReceiver: ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is wrong here
            ScopeImpl::NewScopeType,
            ScopeImpl::NewReceiver,
        >,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
{
    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }

    fn set_done(self) {
        self.nested.set_done();
    }
}

impl<'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value> ReceiverOf<Sch, Value>
    for LetValueReceiver<'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value>
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
    NestedReceiver:
        ReceiverOf<Out::Scheduler, <(Value, <Out as TypedSender>::Value) as TupleCat>::Output>,
    FnType: 'a + BiFunctor<'a, Sch, Value, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    Out: TypedSender
        + TypedSenderConnect<
            'a, // XXX 'a is wrong here
            ScopeImpl::NewScopeType,
            ScopeImpl::NewReceiver,
        >,
    Out::Value: 'a,
    (Value, <Out as TypedSender>::Value): TupleCat,
    <(Value, <Out as TypedSender>::Value) as TupleCat>::Output: 'a + Tuple,
{
    fn set_value(self, sch: Sch, value: Value) {
        let local_receiver_with_value = LocalReceiverWithValue::new(value, self.nested);
        let (local_scope, local_receiver, rcv_ref) =
            self.scope.new_scope(local_receiver_with_value);

        let mut values_ref = ScopedRefMut::map(
            rcv_ref,
            |rcv: &mut LocalReceiverWithValue<
                <Out as TypedSender>::Scheduler,
                Value,
                <Out as TypedSender>::Value,
                NestedReceiver,
            >| rcv.values_ref(),
        );
        let values_ref: &mut Value = unsafe {
            // Might want to not do this... instead pass the values_ref straight to the function.
            // But for that, the values_ref needs to have less associated template-arguments.
            // 'env and State need to get lost, so the type becomes `ScopedRefMut<'scope, Value>`.
            mem::transmute::<&mut Value, &mut Value>(&mut *values_ref)
        };

        match self.fn_impl.tuple_invoke(sch, values_ref) {
            Ok(local_sender) => local_sender.connect(&local_scope, local_receiver).start(),
            Err(error) => local_receiver.set_error(error),
        };
    }
}

struct LocalReceiverWithValue<Sch, Value, OutValue, Rcv>
where
    Sch: Scheduler,
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
    Sch: Scheduler,
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
    Sch: Scheduler,
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
    Sch: Scheduler,
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
