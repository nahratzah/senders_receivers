use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::stop_token::{NeverStopToken, StopToken};
use crate::stop_token::{StopSource, StoppableToken};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use crate::tuple::{Tuple, TupleCat};
use std::marker::PhantomData;
use std::ops::BitOr;
use std::sync::{Arc, Mutex};

/// Combine multiple sender-chains into a single sender-chain.
///
/// Returns a [TypedSender], which completes when all the nested senders complete.
/// The value-signal is the concatenation of value-signals of each of the nested senders.
///
/// If any of the senders yields an error, the returned [TypedSender] will also yield an error-signal.
/// The first received error is forwarded, it's non-deterministic which happens to win the race.
/// In this case, the [StopToken] passed to the wrapped sender-chains is marked as `stopped`, so that the other sender-chains can terminate early.
///
/// If no sender yields an error, and any sender completes with the done signal, the returned [TypedSender] will also yield a done-signal.
/// In this case, the [StopToken] passed to the wrapped sender-chains is marked as `stopped`, so that the other sender-chains can terminate early.
///
/// Only if all senders complete with a value, will the returned [TypedSender] yield a value.
///
/// Even if an error or done signal is received, the code will wait for all senders to complete.
///
/// # Example
/// ```
/// use senders_receivers::{when_all_transfer, Just, SyncWaitSend};
/// use threadpool::ThreadPool;
///
/// let pool = threadpool::ThreadPool::with_name("example".into(), 1);
/// assert_eq!(
///     (1, 2, 3, 4),
///     when_all_transfer!(pool, Just::from((1, 2)), Just::from((3, 4))).sync_wait_send().unwrap().unwrap());
/// ```
///
/// ```
/// use senders_receivers::{when_all_transfer, Just, SyncWaitSend};
/// use threadpool::ThreadPool;
///
/// let pool = threadpool::ThreadPool::with_name("example".into(), 1);
/// assert_eq!(
///     (1, 2, 3, 4, 5, 6, 7, 8, 9),
///     when_all_transfer!(
///         pool,
///         Just::from((1, 2)),
///         Just::from((3, 4)),
///         Just::from((5, 6)),
///         Just::from((7, 8)),
///         Just::from((9,)),
///     ).sync_wait_send().unwrap().unwrap());
/// ```
///
/// In theory, the macro should also work if you invoke it via the crate-name.
/// ```
/// use senders_receivers;
/// use senders_receivers::{Just, SyncWaitSend};
/// use threadpool::ThreadPool;
///
/// let pool = threadpool::ThreadPool::with_name("example".into(), 1);
/// assert_eq!(
///     (1, 2, 3, 4, 5, 6, 7, 8, 9),
///     senders_receivers::when_all_transfer!(
///         pool,
///         Just::from((1, 2)),
///         Just::from((3, 4)),
///         Just::from((5, 6)),
///         Just::from((7, 8)),
///         Just::from((9,)),
///     ).sync_wait_send().unwrap().unwrap());
/// ```
///
/// Unlike [when_all!](crate::when_all!), the macro also works if you have no senders:
/// ```
/// use senders_receivers::{when_all_transfer, SyncWaitSend};
/// use threadpool::ThreadPool;
///
/// let pool = threadpool::ThreadPool::with_name("example".into(), 1);
/// assert_eq!(
///     (),
///     when_all_transfer!(pool).sync_wait_send().unwrap().unwrap());
/// ```
#[macro_export]
macro_rules! when_all_transfer {
    ($scheduler:expr $(,)?) => {{
        use senders_receivers::Scheduler;
        $scheduler.schedule()
    }};
    ($scheduler:expr, $typed_sender_0:expr, $($senders:expr),* $(,)?) => {{
        use senders_receivers;
        use senders_receivers::NoSchedulerSenderValue;

        let stop_source = senders_receivers::stop_token::StopSource::default();
        senders_receivers::NoSchedulerSenderImpl::new(stop_source.clone(), $typed_sender_0)
            $(.cat(senders_receivers::NoSchedulerSenderImpl::new(stop_source.clone(), $senders)))*
            .schedule(stop_source, $scheduler)
    }};
}

/// A [TypedSender] trait, except it lacks a [Scheduler](TypedSender::Scheduler) type.
///
/// It also implements the functions required by the [when_all_transfer!] macro.
pub trait NoSchedulerSenderValue {
    /// The value type that this sender will create.
    type Value: Tuple;

    /// Attach a new [NoSchedulerSenderValue] to this one, creating a sender that'll yield the tuple-concatenation of the values of each.
    fn cat<TS>(self, ts: TS) -> PairwiseTS<Self, TS>
    where
        Self: Sized,
        TS: NoSchedulerSenderValue,
        (Self::Value, TS::Value): TupleCat,
        <(Self::Value, TS::Value) as TupleCat>::Output: Tuple,
    {
        PairwiseTS::new(self, ts)
    }

    /// Attach the scheduler type.
    fn schedule<Sch>(self, stop_source: StopSource, sch: Sch) -> SchedulerTS<Sch, Self>
    where
        Self: Sized,
        Sch: Scheduler,
    {
        SchedulerTS::new(sch, stop_source, self)
    }
}

/// A [TypedSenderConnect] trait, except it lacks a [Scheduler](TypedSender::Scheduler) type.
pub trait NoSchedulerSender<'a, ScopeImpl, ReceiverType>: NoSchedulerSenderValue
where
    ReceiverType: NoSchedulerReceiver<<Self as NoSchedulerSenderValue>::Value>,
{
    /// The [OperationState] returned by [NoSchedulerSender::connect()].
    type Output<'scope>: 'scope + OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;

    /// Connect a [NoSchedulerReceiver] to this sender.
    fn connect<'scope>(self, scope: &ScopeImpl, rcv: ReceiverType) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;
}

/// A receiver for [NoSchedulerSender].
///
/// It has a mutable-reference receiver, so it can be used in shared context.
pub trait NoSchedulerReceiver<Value>
where
    Value: Tuple,
{
    /// Complete this receiver with a value-signal.
    fn set_value(&mut self, value: Value);
    /// Complete this receiver with an error-signal.
    fn set_error(&mut self, error: Error);
    /// Complete this receiver with a done-signal.
    fn set_done(&mut self);
}

/// Take a regular scheduler, and turn it into a [NoSchedulerSender].
pub struct NoSchedulerSenderImpl<TS>
where
    TS: TypedSender,
{
    stop_source: StopSource,
    sender: TS,
}

impl<TS> NoSchedulerSenderImpl<TS>
where
    TS: TypedSender,
{
    /// Instantiate a [NoSchedulerSenderImpl].
    ///
    /// The [StopSource] is used to [StopSource::request_stop] if a `done` or `error` signal is emitted by the wrapped [TypedSender].
    /// The [StopSource.token] is used passed to the [TypedSenderConnect::connect] method.
    pub fn new(stop_source: StopSource, sender: TS) -> Self {
        Self {
            stop_source,
            sender,
        }
    }
}

impl<TS> NoSchedulerSenderValue for NoSchedulerSenderImpl<TS>
where
    TS: TypedSender,
{
    type Value = TS::Value;
}

impl<'a, ScopeImpl, ReceiverType, TS> NoSchedulerSender<'a, ScopeImpl, ReceiverType>
    for NoSchedulerSenderImpl<TS>
where
    ReceiverType: NoSchedulerReceiver<<Self as NoSchedulerSenderValue>::Value>,
    TS: TypedSender
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            StoppableToken,
            Wrap<ReceiverType, <TS as TypedSender>::Value>,
        >,
    <TS as TypedSender>::Value: 'a,
{
    type Output<'scope> = TS::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(self, scope: &ScopeImpl, rcv: ReceiverType) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        let stop_token = self.stop_source.token();
        self.sender
            .connect(scope, stop_token, Wrap::new(self.stop_source, rcv))
    }
}

/// Wrap a [NoSchedulerReceiver] into a regular [Receiver].
pub struct Wrap<Rcv, Value>
where
    Rcv: NoSchedulerReceiver<Value>,
    Value: Tuple,
{
    phantom: PhantomData<fn(Value)>,
    stop_source: StopSource,
    rcv: Rcv,
}

impl<Rcv, Value> Wrap<Rcv, Value>
where
    Rcv: NoSchedulerReceiver<Value>,
    Value: Tuple,
{
    fn new(stop_source: StopSource, rcv: Rcv) -> Self {
        Self {
            phantom: PhantomData,
            stop_source,
            rcv,
        }
    }
}

impl<Rcv, Value> Receiver for Wrap<Rcv, Value>
where
    Rcv: NoSchedulerReceiver<Value>,
    Value: Tuple,
{
    fn set_error(mut self, error: Error) {
        self.stop_source.request_stop();
        self.rcv.set_error(error)
    }

    fn set_done(mut self) {
        self.stop_source.request_stop();
        self.rcv.set_done()
    }
}

impl<Rcv, Sch, Value> ReceiverOf<Sch, Value> for Wrap<Rcv, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Rcv: NoSchedulerReceiver<Value>,
    Value: Tuple,
{
    fn set_value(mut self, _: Sch, value: Value) {
        self.rcv.set_value(value)
    }
}

/// [NoSchedulerSender] that combines two of them into one.
pub struct PairwiseTS<X, Y>
where
    X: NoSchedulerSenderValue,
    Y: NoSchedulerSenderValue,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    x: X,
    y: Y,
}

impl<X, Y> PairwiseTS<X, Y>
where
    X: NoSchedulerSenderValue,
    Y: NoSchedulerSenderValue,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    fn new(x: X, y: Y) -> Self {
        Self { x, y }
    }
}

impl<X, Y> NoSchedulerSenderValue for PairwiseTS<X, Y>
where
    X: NoSchedulerSenderValue,
    Y: NoSchedulerSenderValue,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    type Value = <(X::Value, Y::Value) as TupleCat>::Output;
}

impl<'a, ScopeImpl, ReceiverType, X, Y> NoSchedulerSender<'a, ScopeImpl, ReceiverType>
    for PairwiseTS<X, Y>
where
    X: NoSchedulerSenderValue
        + NoSchedulerSender<
            'a,
            ScopeImpl,
            XSplitReceiver<
                ReceiverType,
                <X as NoSchedulerSenderValue>::Value,
                <Y as NoSchedulerSenderValue>::Value,
            >,
        >,
    Y: NoSchedulerSenderValue
        + NoSchedulerSender<
            'a,
            ScopeImpl,
            YSplitReceiver<
                ReceiverType,
                <X as NoSchedulerSenderValue>::Value,
                <Y as NoSchedulerSenderValue>::Value,
            >,
        >,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
    ReceiverType: NoSchedulerReceiver<Self::Value>,
    X::Value: 'a,
    Y::Value: 'a,
{
    type Output<'scope> = WhenAllOperationState<
        'scope,
	X::Output<'scope>,
	Y::Output<'scope>,
    >
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(self, scope: &ScopeImpl, rcv: ReceiverType) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        let rcv = Arc::new(Mutex::new(SplitReceiver::from(rcv)));
        let x_opstate = self.x.connect(scope, XSplitReceiver::from(rcv.clone()));
        let y_opstate = self.y.connect(scope, YSplitReceiver::from(rcv));
        WhenAllOperationState::new(x_opstate, y_opstate)
    }
}

/// Split a single receiver into two components.
struct SplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    error: Option<Error>,
    x: Option<XValue>,
    y: Option<YValue>,
    done: bool,
    pending: u8,
    rcv: Arc<Mutex<Rcv>>,
}

impl<Rcv, XValue, YValue> From<Rcv> for SplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    fn from(rcv: Rcv) -> Self {
        Self {
            error: None,
            x: None,
            y: None,
            done: false,
            pending: 2,
            rcv: Arc::new(Mutex::new(rcv)),
        }
    }
}

impl<Rcv, XValue, YValue> SplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    fn invariant(&self) {
        if self.pending == 0 {
            panic!("should have completed")
        }
    }

    fn ready(&self) -> bool {
        self.pending == 0
    }

    fn maybe_complete(&mut self) {
        self.pending -= 1;

        if self.ready() {
            let mut rcv = self.rcv.lock().unwrap();
            if let Some(error) = self.error.take() {
                rcv.set_error(error);
            } else if self.done {
                rcv.set_done();
            } else if let (Some(x), Some(y)) = (self.x.take(), self.y.take()) {
                rcv.set_value((x, y).cat());
            } else {
                unreachable!();
            }
        }
    }
}

pub struct XSplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    ptr: Arc<Mutex<SplitReceiver<Rcv, XValue, YValue>>>,
}

impl<Rcv, XValue, YValue> From<Arc<Mutex<SplitReceiver<Rcv, XValue, YValue>>>>
    for XSplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    fn from(ptr: Arc<Mutex<SplitReceiver<Rcv, XValue, YValue>>>) -> Self {
        Self { ptr }
    }
}

impl<Rcv, XValue, YValue> NoSchedulerReceiver<XValue> for XSplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    fn set_value(&mut self, value: XValue) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.x.is_some() {
            panic!("value already assigned");
        }

        let _ = r.x.insert(value);
        r.maybe_complete();
    }

    fn set_error(&mut self, error: Error) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.x.is_some() {
            panic!("value already assigned");
        }

        let _ = r.error.get_or_insert(error);
        r.maybe_complete();
    }

    fn set_done(&mut self) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.x.is_some() {
            panic!("value already assigned");
        }

        r.done = true;
        r.maybe_complete();
    }
}

pub struct YSplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    ptr: Arc<Mutex<SplitReceiver<Rcv, XValue, YValue>>>,
}

impl<Rcv, XValue, YValue> From<Arc<Mutex<SplitReceiver<Rcv, XValue, YValue>>>>
    for YSplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    fn from(ptr: Arc<Mutex<SplitReceiver<Rcv, XValue, YValue>>>) -> Self {
        Self { ptr }
    }
}

impl<Rcv, XValue, YValue> NoSchedulerReceiver<YValue> for YSplitReceiver<Rcv, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
{
    fn set_value(&mut self, value: YValue) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.y.is_some() {
            panic!("value already assigned");
        }

        let _ = r.y.insert(value);
        r.maybe_complete();
    }

    fn set_error(&mut self, error: Error) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.y.is_some() {
            panic!("value already assigned");
        }

        let _ = r.error.get_or_insert(error);
        r.maybe_complete();
    }

    fn set_done(&mut self) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.y.is_some() {
            panic!("value already assigned");
        }

        r.done = true;
        r.maybe_complete();
    }
}

/// A [TypedSender] that'll attach a [Scheduler] to the value of the contained [NoSchedulerSender].
pub struct SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sender: NoSchedulerSenderValue,
{
    sch: Sch,
    stop_source: StopSource,
    sender: Sender,
}

impl<Sch, Sender> SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sender: NoSchedulerSenderValue,
{
    fn new(sch: Sch, stop_source: StopSource, sender: Sender) -> Self {
        Self {
            sch,
            stop_source,
            sender,
        }
    }
}

impl<Sch, Sender> TypedSender for SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sender: NoSchedulerSenderValue,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = Sender::Value;
}

impl<'a, ScopeImpl, StopTokenImpl, Rcv, Sch, Sender>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, Rcv> for SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sch::Sender: for<'b> TypedSenderConnect<
        'b,
        ScopeImpl,
        NeverStopToken,
        WrapValue<<Sender as NoSchedulerSenderValue>::Value, Rcv>,
    >,
    Sender: NoSchedulerSenderValue
        + NoSchedulerSender<'a, ScopeImpl, SchedulerReceiver<ScopeImpl, Sch, Rcv>>,
    ScopeImpl: Clone,
    StopTokenImpl: StopToken,
    Rcv: ReceiverOf<Sch::LocalScheduler, Sender::Value>,
{
    type Output<'scope> = Sender::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope;

    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        stop_token: StopTokenImpl,
        rcv: Rcv,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope,
    {
        if StopTokenImpl::STOP_POSSIBLE {
            let stop_source = self.stop_source;
            if let Err(f) = stop_token.detached_callback(move || stop_source.request_stop()) {
                // Already canceled, so just cancel the stop_source now.
                f();
            }
        }

        let rcv = SchedulerReceiver::new(scope.clone(), self.sch, rcv);
        self.sender.connect(scope, rcv)
    }
}

impl<Sch, Sender, BindSenderImpl> BitOr<BindSenderImpl> for SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sender: NoSchedulerSenderValue,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct SchedulerReceiver<Scope, Sch, Rcv>
where
    Sch: Scheduler,
    Rcv: Receiver,
{
    scope: Scope,
    sch: Sch,
    rcv: Option<Rcv>,
}

impl<Scope, Sch, Rcv> SchedulerReceiver<Scope, Sch, Rcv>
where
    Sch: Scheduler,
    Rcv: Receiver,
{
    fn new(scope: Scope, sch: Sch, rcv: Rcv) -> Self {
        Self {
            scope,
            sch,
            rcv: Some(rcv),
        }
    }
}

impl<Scope, Sch, Value, Rcv> NoSchedulerReceiver<Value> for SchedulerReceiver<Scope, Sch, Rcv>
where
    Sch: Scheduler,
    Sch::Sender: for<'a> TypedSenderConnect<'a, Scope, NeverStopToken, WrapValue<Value, Rcv>>,
    Value: Tuple,
    Rcv: Receiver + ReceiverOf<Sch::LocalScheduler, Value>,
{
    fn set_error(&mut self, error: Error) {
        self.rcv
            .take()
            .expect("receiver has not yet completed")
            .set_error(error);
    }

    fn set_done(&mut self) {
        self.rcv
            .take()
            .expect("receiver has not yet completed")
            .set_done();
    }

    fn set_value(&mut self, value: Value) {
        self.sch
            .schedule()
            .connect(
                &self.scope,
                NeverStopToken,
                WrapValue::new(
                    value,
                    self.rcv.take().expect("receiver has not yet completed"),
                ),
            )
            .start();
    }
}

struct WrapValue<Value, Rcv>
where
    Value: Tuple,
    Rcv: Receiver,
{
    value: Value,
    rcv: Rcv,
}

impl<Value, Rcv> WrapValue<Value, Rcv>
where
    Value: Tuple,
    Rcv: Receiver,
{
    fn new(value: Value, rcv: Rcv) -> Self {
        Self { value, rcv }
    }
}

impl<Value, Rcv> Receiver for WrapValue<Value, Rcv>
where
    Value: Tuple,
    Rcv: Receiver,
{
    fn set_error(self, error: Error) {
        self.rcv.set_error(error);
    }

    fn set_done(self) {
        self.rcv.set_done();
    }
}

impl<Sch, Value, Rcv> ReceiverOf<Sch, ()> for WrapValue<Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn set_value(self, sch: Sch, _: ()) {
        self.rcv.set_value(sch, self.value);
    }
}

pub struct WhenAllOperationState<'scope, X, Y>
where
    X: OperationState<'scope>,
    Y: OperationState<'scope>,
{
    phantom: PhantomData<&'scope ()>,
    x: X,
    y: Y,
}

impl<'scope, X, Y> WhenAllOperationState<'scope, X, Y>
where
    X: OperationState<'scope>,
    Y: OperationState<'scope>,
{
    fn new(x: X, y: Y) -> Self {
        Self {
            phantom: PhantomData,
            x,
            y,
        }
    }
}

impl<'scope, X, Y> OperationState<'scope> for WhenAllOperationState<'scope, X, Y>
where
    X: OperationState<'scope>,
    Y: OperationState<'scope>,
{
    fn start(self) {
        self.x.start();
        self.y.start();
    }
}

#[cfg(test)]
mod tests {
    use super::NoSchedulerSenderImpl;
    use super::NoSchedulerSenderValue;
    use crate::errors::{new_error, ErrorForTesting};
    use crate::just::Just;
    use crate::just_done::JustDone;
    use crate::just_error::JustError;
    use crate::scheduler::ImmediateScheduler;
    use crate::stop_token::StopSource;
    use crate::sync_wait::{SyncWait, SyncWaitSend};
    use threadpool::ThreadPool;

    #[test]
    fn it_works() {
        let stop_source = StopSource::default();
        let sender = NoSchedulerSenderImpl::new(stop_source.clone(), Just::from((1, 2)))
            .cat(NoSchedulerSenderImpl::new(
                stop_source.clone(),
                Just::from((3, 4)),
            ))
            .schedule(stop_source, ImmediateScheduler);
        assert_eq!((1, 2, 3, 4), sender.sync_wait().unwrap().unwrap());
    }

    #[test]
    fn it_works_with_threadpool() {
        let stop_source = StopSource::default();
        let pool = ThreadPool::with_name("it_works_with_threadpool".into(), 2);
        let sender = NoSchedulerSenderImpl::new(stop_source.clone(), Just::from((1, 2)))
            .cat(NoSchedulerSenderImpl::new(
                stop_source.clone(),
                Just::from((3, 4)),
            ))
            .schedule(stop_source, pool);
        assert_eq!((1, 2, 3, 4), sender.sync_wait_send().unwrap().unwrap());
    }

    #[test]
    fn errors_are_propagated() {
        let stop_source = StopSource::default();

        let outcome = NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(ErrorForTesting::from(
                "error",
            ))),
        )
        .cat(NoSchedulerSenderImpl::new(
            stop_source.clone(),
            Just::from((3, 4)),
        ))
        .schedule(stop_source, ImmediateScheduler)
        .sync_wait();

        match outcome {
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
    fn done_is_propagated() {
        let stop_source = StopSource::default();

        let outcome = NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustDone::<ImmediateScheduler, (i32, i32)>::new(),
        )
        .cat(NoSchedulerSenderImpl::new(
            stop_source.clone(),
            Just::from((3, 4)),
        ))
        .schedule(stop_source, ImmediateScheduler)
        .sync_wait();

        match outcome {
            Ok(None) => {}
            _ => {
                panic!("expected cancelation");
            }
        }
    }

    #[test]
    fn errors_are_propagated_even_when_done_signal_is_present() {
        let stop_source = StopSource::default();

        let outcome = NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustDone::<ImmediateScheduler, (i32, i32)>::new(),
        )
        .cat(NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(ErrorForTesting::from(
                "error",
            ))),
        ))
        .schedule(stop_source, ImmediateScheduler)
        .sync_wait();

        match outcome {
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
