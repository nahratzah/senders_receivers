use crate::errors::{Error, Result};
use crate::functor::{BiClosure, BiFunctor, NoErrBiFunctor};
use crate::refs::ScopedRefMut;
use crate::scheduler::Scheduler;
use crate::scope::ScopeNest;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use crate::tuple::DistributeRefTuple;
use crate::tuple::Tuple;
use crate::tuple::TupleCat;
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
///              | LetValue::from(|_, (name,)| {
///                  Just::from((format!("Hello {}!", name),))
///              });
/// assert_eq!(
///     (String::from("world"), String::from("Hello world!")),
///     sender.sync_wait().unwrap().unwrap());
///
/// // If using a function that returns a Result:
/// let sender = Just::from((String::from("world"),))
///              | LetValue::from(|_, (name,)| {
///                  Ok(Just::from((format!("Hello {}!", name),)))
///              });
/// assert_eq!(
///     (String::from("world"), String::from("Hello world!")),
///     sender.sync_wait().unwrap().unwrap());
/// ```
pub struct LetValue<'scope, 'a, FnType, Out, Sch, Value>
where
    'a: 'scope,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn_impl: FnType,
    phantom: PhantomData<(&'a (), &'scope (), fn(Sch, Value) -> Out)>,
}

impl<'scope, 'a, FnType, Out, Sch, Value> From<FnType>
    for LetValue<'scope, 'a, FnType, Out, Sch, Value>
where
    'a: 'scope,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
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

type ClosureLetValue<'scope, 'a, FnType, Out, Sch, Value> = LetValue<
    'scope,
    'a,
    BiClosure<'a, FnType, Result<Out>, Sch, <&'scope mut Value as DistributeRefTuple>::Output>,
    Out,
    Sch,
    Value,
>;

impl<'scope, 'a, FnType, Out, Sch, Value> From<FnType>
    for ClosureLetValue<'scope, 'a, FnType, Out, Sch, Value>
where
    'a: 'scope,
    FnType: 'a + FnOnce(Sch, <&'scope mut Value as DistributeRefTuple>::Output) -> Result<Out>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

type NoErrLetValue<'scope, 'a, FnType, Out, Sch, Value> = LetValue<
    'scope,
    'a,
    NoErrBiFunctor<'a, FnType, Out, Sch, <&'scope mut Value as DistributeRefTuple>::Output>,
    Out,
    Sch,
    Value,
>;

impl<'scope, 'a, FnType, Out, Sch, Value> From<FnType>
    for NoErrLetValue<'scope, 'a, FnType, Out, Sch, Value>
where
    'a: 'scope,
    FnType: BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Out>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(NoErrBiFunctor::new(fn_impl))
    }
}

type NoErrClosureLetValue<'scope, 'a, FnType, Out, Sch, Value> = NoErrLetValue<
    'scope,
    'a,
    BiClosure<'a, FnType, Out, Sch, <&'scope mut Value as DistributeRefTuple>::Output>,
    Out,
    Sch,
    Value,
>;

impl<'scope, 'a, FnType, Out, Sch, Value> From<FnType>
    for NoErrClosureLetValue<'scope, 'a, FnType, Out, Sch, Value>
where
    'a: 'scope,
    FnType: 'a + FnOnce(Sch, <&'scope mut Value as DistributeRefTuple>::Output) -> Out,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    fn from(fn_impl: FnType) -> Self {
        Self::from(BiClosure::new(fn_impl))
    }
}

impl<'scope, 'a, FnType, Out, Sch, Value> Sender for LetValue<'scope, 'a, FnType, Out, Sch, Value>
where
    'a: 'scope,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
}

impl<'scope, 'a, NestedSender, FnType, Out, Sch, Value> BindSender<NestedSender>
    for LetValue<'scope, 'a, FnType, Out, Sch, Value>
where
    'a: 'scope,
    NestedSender: TypedSender<'a, Scheduler = Sch, Value = Value>,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    type Output = LetValueTS<'scope, 'a, NestedSender, FnType, Out, Sch, Value>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        LetValueTS {
            nested,
            fn_impl: self.fn_impl,
            phantom: PhantomData,
        }
    }
}

pub struct LetValueTS<'scope, 'a, NestedSender, FnType, Out, Sch, Value>
where
    'a: 'scope,
    NestedSender: TypedSender<'a, Scheduler = Sch, Value = Value>,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom: PhantomData<(&'a (), &'scope (), fn(Sch, Value) -> Out)>,
}

impl<'scope, 'a, BindSenderImpl, NestedSender, FnType, Out, Sch, Value> BitOr<BindSenderImpl>
    for LetValueTS<'scope, 'a, NestedSender, FnType, Out, Sch, Value>
where
    BindSenderImpl: BindSender<Self>,
    'a: 'scope,
    NestedSender: TypedSender<'a, Scheduler = Sch, Value = Value>,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<'scope, 'a, NestedSender, FnType, Out, Sch, Value> TypedSender<'a>
    for LetValueTS<'scope, 'a, NestedSender, FnType, Out, Sch, Value>
where
    'a: 'scope,
    NestedSender: TypedSender<'a, Scheduler = Sch, Value = Value>,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope + TypedSender<'scope>,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: Tuple,
{
    type Value = <(Value, Out::Value) as TupleCat>::Output;
    type Scheduler = Out::Scheduler;
}

impl<'scope, 'a, ScopeImpl, ReceiverImpl, NestedSender, FnType, Out, Sch, Value>
    TypedSenderConnect<'scope, 'a, ScopeImpl, ReceiverImpl>
    for LetValueTS<'scope, 'a, NestedSender, FnType, Out, Sch, Value>
where
    'a: 'scope,
    NestedSender: TypedSender<'a, Scheduler = Sch, Value = Value>
        + TypedSenderConnect<
            'scope,
            'a,
            ScopeImpl,
            LetValueReceiver<'scope, 'a, ScopeImpl, ReceiverImpl, FnType, Out, Sch, Value>,
        >,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope
        + TypedSender<'scope>
        + for<'nested_scope> TypedSenderConnect<
            'nested_scope,
            'scope,
            ScopeImpl::NewScopeType<'nested_scope>,
            ScopeImpl::NewReceiver<'nested_scope>,
        >,
    Out::Value: 'a,
    ScopeImpl: 'scope
        + ScopeNest<
            <Out as TypedSender<'scope>>::Scheduler,
            <Out as TypedSender<'scope>>::Value,
            LocalReceiverWithValue<
                'scope,
                'a,
                <Out as TypedSender<'scope>>::Scheduler,
                Value,
                <Out as TypedSender<'scope>>::Value,
                ReceiverImpl,
            >,
        >,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
    ReceiverImpl: 'scope + ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
{
    fn connect(self, scope: &ScopeImpl, receiver: ReceiverImpl) -> impl OperationState<'scope> {
        let receiver = LetValueReceiver {
            phantom: PhantomData,
            nested: receiver,
            scope: scope.clone(),
            fn_impl: self.fn_impl,
        };
        self.nested.connect(scope, receiver)
    }
}

struct LetValueReceiver<'scope, 'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value>
where
    'a: 'scope,
    ScopeImpl: ScopeNest<
        <Out as TypedSender<'scope>>::Scheduler,
        <Out as TypedSender<'scope>>::Value,
        LocalReceiverWithValue<
            'scope,
            'a,
            <Out as TypedSender<'scope>>::Scheduler,
            Value,
            <Out as TypedSender<'scope>>::Value,
            NestedReceiver,
        >,
    >,
    NestedReceiver: 'scope + ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope
        + TypedSender<'scope>
        + for<'nested_scope> TypedSenderConnect<
            'nested_scope,
            'scope,
            ScopeImpl::NewScopeType<'nested_scope>,
            ScopeImpl::NewReceiver<'nested_scope>,
        >,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
{
    phantom: PhantomData<(&'a (), &'scope (), fn(Sch, Value) -> Out)>,
    nested: NestedReceiver,
    scope: ScopeImpl,
    fn_impl: FnType,
}

impl<'scope, 'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value> Receiver
    for LetValueReceiver<'scope, 'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value>
where
    'a: 'scope,
    ScopeImpl: ScopeNest<
        <Out as TypedSender<'scope>>::Scheduler,
        <Out as TypedSender<'scope>>::Value,
        LocalReceiverWithValue<
            'scope,
            'a,
            <Out as TypedSender<'scope>>::Scheduler,
            Value,
            <Out as TypedSender<'scope>>::Value,
            NestedReceiver,
        >,
    >,
    NestedReceiver: 'scope + ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope
        + TypedSender<'scope>
        + for<'nested_scope> TypedSenderConnect<
            'nested_scope,
            'scope,
            ScopeImpl::NewScopeType<'nested_scope>,
            ScopeImpl::NewReceiver<'nested_scope>,
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

impl<'scope, 'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value> ReceiverOf<Sch, Value>
    for LetValueReceiver<'scope, 'a, ScopeImpl, NestedReceiver, FnType, Out, Sch, Value>
where
    'a: 'scope,
    ScopeImpl: ScopeNest<
        <Out as TypedSender<'scope>>::Scheduler,
        <Out as TypedSender<'scope>>::Value,
        LocalReceiverWithValue<
            'scope,
            'a,
            <Out as TypedSender<'scope>>::Scheduler,
            Value,
            <Out as TypedSender<'scope>>::Value,
            NestedReceiver,
        >,
    >,
    NestedReceiver: 'scope + ReceiverOf<Out::Scheduler, <(Value, Out::Value) as TupleCat>::Output>,
    FnType: 'a
        + BiFunctor<'a, Sch, <&'scope mut Value as DistributeRefTuple>::Output, Output = Result<Out>>,
    Sch: Scheduler,
    Value: 'a + Tuple,
    &'scope mut Value: DistributeRefTuple,
    Out: 'scope
        + TypedSender<'scope>
        + for<'nested_scope> TypedSenderConnect<
            'nested_scope,
            'scope,
            ScopeImpl::NewScopeType<'nested_scope>,
            ScopeImpl::NewReceiver<'nested_scope>,
        >,
    Out::Value: 'a,
    (Value, Out::Value): TupleCat,
    <(Value, Out::Value) as TupleCat>::Output: 'a + Tuple,
{
    fn set_value(self, sch: Sch, value: Value) {
        let local_receiver_with_value = LocalReceiverWithValue::new(value, self.nested);
        let (local_scope, local_receiver, rcv_ref) =
            self.scope.new_scope(local_receiver_with_value);

        let mut values_ref = ScopedRefMut::map(
            rcv_ref,
            |rcv: &mut LocalReceiverWithValue<
                'scope,
                'a,
                <Out as TypedSender<'scope>>::Scheduler,
                Value,
                <Out as TypedSender<'scope>>::Value,
                NestedReceiver,
            >| rcv.values_ref(),
        );
        let values_ref: &'scope mut Value = unsafe {
            // Might want to not do this... instead pass the values_ref straight to the function.
            // But for that, the values_ref needs to have less associated template-arguments.
            // 'env and State need to get lost, so the type becomes `ScopedRefMut<'scope, Value>`.
            mem::transmute::<&mut Value, &'scope mut Value>(&mut *values_ref)
        };
        let values_ref = DistributeRefTuple::distribute(values_ref);

        match self.fn_impl.tuple_invoke(sch, values_ref) {
            Ok(local_sender) => local_sender.connect(&local_scope, local_receiver).start(),
            Err(error) => local_receiver.set_error(error),
        };
    }
}

struct LocalReceiverWithValue<'scope, 'a, Sch, Value, OutValue, Rcv>
where
    'a: 'scope,
    Sch: Scheduler,
    Value: 'a + Tuple,
    OutValue: 'a + Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: 'a + Tuple,
    Rcv: 'scope + ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
{
    phantom: PhantomData<(&'a (), &'scope (), fn(Sch, OutValue))>,
    value: Box<Value>,
    rcv: Rcv,
}

impl<'scope, 'a, Sch, Value, OutValue, Rcv>
    LocalReceiverWithValue<'scope, 'a, Sch, Value, OutValue, Rcv>
where
    'a: 'scope,
    Sch: Scheduler,
    Value: 'a + Tuple,
    OutValue: 'a + Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: 'a + Tuple,
    Rcv: 'scope + ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
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

impl<'scope, 'a, Sch, Value, OutValue, Rcv> Receiver
    for LocalReceiverWithValue<'scope, 'a, Sch, Value, OutValue, Rcv>
where
    'a: 'scope,
    Sch: Scheduler,
    Value: 'a + Tuple,
    OutValue: 'a + Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: 'a + Tuple,
    Rcv: 'scope + ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
{
    fn set_error(self, error: Error) {
        self.rcv.set_error(error);
    }

    fn set_done(self) {
        self.rcv.set_done();
    }
}

impl<'scope, 'a, Sch, Value, OutValue, Rcv> ReceiverOf<Sch, OutValue>
    for LocalReceiverWithValue<'scope, 'a, Sch, Value, OutValue, Rcv>
where
    'a: 'scope,
    Sch: Scheduler,
    Value: 'a + Tuple,
    OutValue: 'a + Tuple,
    (Value, OutValue): TupleCat,
    <(Value, OutValue) as TupleCat>::Output: 'a + Tuple,
    Rcv: 'scope + ReceiverOf<Sch, <(Value, OutValue) as TupleCat>::Output>,
{
    fn set_value(self, sch: Sch, value: OutValue) {
        self.rcv.set_value(sch, (*self.value, value).cat());
    }
}
