use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::stop_token::{StopSource, StopToken, StoppableToken};
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
///
/// If no sender yields an error, and any sender completes with the done signal, the returned [TypedSender] will also yield a done-signal.
///
/// Only if all senders complete with a value, will the returned [TypedSender] yield a value.
///
/// Even if an error or done signal is received, the code will wait for all senders to complete.
///
/// # Note
/// The `when_all!` macro does not transfer scheduler, and therefore requires that all of its senders use the same scheduler type.
/// If you need to combine multiple senders that use different schedulers, consider using [when_all_transfer!](crate::when_all_transfer!).
///
/// # Example
/// ```
/// use senders_receivers::{when_all, Just, SyncWait};
///
/// assert_eq!(
///     (1, 2, 3, 4),
///     when_all!(Just::from((1, 2)), Just::from((3, 4))).sync_wait().unwrap().unwrap());
/// ```
///
/// ```
/// use senders_receivers::{when_all, Just, SyncWait};
///
/// assert_eq!(
///     (1, 2, 3, 4, 5, 6, 7, 8, 9),
///     when_all!(
///         Just::from((1, 2)),
///         Just::from((3, 4)),
///         Just::from((5, 6)),
///         Just::from((7, 8)),
///         Just::from((9,)),
///     ).sync_wait().unwrap().unwrap());
/// ```
///
/// In theory, the macro should also work if you invoke it via the crate-name.
/// ```
/// use senders_receivers;
/// use senders_receivers::{Just, SyncWait};
///
/// assert_eq!(
///     (1, 2, 3, 4, 5, 6, 7, 8, 9),
///     senders_receivers::when_all!(
///         Just::from((1, 2)),
///         Just::from((3, 4)),
///         Just::from((5, 6)),
///         Just::from((7, 8)),
///         Just::from((9,)),
///     ).sync_wait().unwrap().unwrap());
/// ```
///
/// Also, when_all should totally work with a single type.
/// ```
/// use senders_receivers;
/// use senders_receivers::{Just, SyncWait};
///
/// assert_eq!(
///     (1, 2),
///     senders_receivers::when_all!(
///         Just::from((1, 2)),
///     ).sync_wait().unwrap().unwrap());
/// ```
#[macro_export]
macro_rules! when_all {
    ($typed_sender:expr $(,)?) => {
        $typed_sender
    };
    ($typed_sender_0:expr, $($senders:expr),+ $(,)?) => {{
        use senders_receivers;
        use senders_receivers::when_all;
        use senders_receivers::when_all::NoSchedulerSenderValue;

        let stop_source = senders_receivers::stop_token::StopSource::default();
        when_all::NoSchedulerSenderImpl::new(stop_source.clone(), $typed_sender_0)
            $(.cat(when_all::NoSchedulerSenderImpl::new(stop_source.clone(), $senders)))+
            .fin(stop_source)
    }};
}

pub trait NoSchedulerSenderValue {
    type Scheduler: Scheduler<LocalScheduler = Self::Scheduler>;
    type Value: Tuple;

    fn cat<NoSchedulerSenderImpl>(
        self,
        rhs: NoSchedulerSenderImpl,
    ) -> PairwiseTS<Self, NoSchedulerSenderImpl>
    where
        Self: Sized,
        NoSchedulerSenderImpl: NoSchedulerSenderValue<Scheduler = Self::Scheduler>,
        (Self::Value, NoSchedulerSenderImpl::Value): TupleCat,
        <(Self::Value, NoSchedulerSenderImpl::Value) as TupleCat>::Output: Tuple,
    {
        PairwiseTS::new(self, rhs)
    }

    fn fin(self, stop_source: StopSource) -> WhenAllFin<Self>
    where
        Self: Sized,
    {
        WhenAllFin::new(stop_source, self)
    }
}

pub trait NoSchedulerSender<'a, ScopeImpl, ReceiverType>: NoSchedulerSenderValue
where
    ReceiverType: NoSchedulerReceiver<
        <Self as NoSchedulerSenderValue>::Scheduler,
        <Self as NoSchedulerSenderValue>::Value,
    >,
{
    type Output<'scope>: 'scope + OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(self, scope: &ScopeImpl, rcv: ReceiverType) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;
}

pub trait NoSchedulerReceiver<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
{
    /// Complete this receiver with a value-signal.
    fn set_value(&mut self, sch: Sch, value: Value);
    /// Complete this receiver with an error-signal.
    fn set_error(&mut self, error: Error);
    /// Complete this receiver with a done-signal.
    fn set_done(&mut self);
}

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
    type Scheduler = TS::Scheduler;
    type Value = TS::Value;
}

impl<'a, ScopeImpl, ReceiverType, TS> NoSchedulerSender<'a, ScopeImpl, ReceiverType>
    for NoSchedulerSenderImpl<TS>
where
    ReceiverType: NoSchedulerReceiver<
        <Self as NoSchedulerSenderValue>::Scheduler,
        <Self as NoSchedulerSenderValue>::Value,
    >,
    TS: TypedSenderConnect<
        'a,
        ScopeImpl,
        StoppableToken,
        Wrap<ReceiverType, <TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Value: 'a,
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

pub struct PairwiseTS<X, Y>
where
    X: NoSchedulerSenderValue,
    Y: NoSchedulerSenderValue<Scheduler = X::Scheduler>,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    x: X,
    y: Y,
}

impl<X, Y> PairwiseTS<X, Y>
where
    X: NoSchedulerSenderValue,
    Y: NoSchedulerSenderValue<Scheduler = X::Scheduler>,
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
    Y: NoSchedulerSenderValue<Scheduler = X::Scheduler>,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    type Scheduler = X::Scheduler;
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
                <X as NoSchedulerSenderValue>::Scheduler,
                <X as NoSchedulerSenderValue>::Value,
                <Y as NoSchedulerSenderValue>::Value,
            >,
        >,
    Y: NoSchedulerSenderValue<Scheduler = X::Scheduler>
        + NoSchedulerSender<
            'a,
            ScopeImpl,
            YSplitReceiver<
                ReceiverType,
                <X as NoSchedulerSenderValue>::Scheduler,
                <X as NoSchedulerSenderValue>::Value,
                <Y as NoSchedulerSenderValue>::Value,
            >,
        >,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
    ReceiverType: NoSchedulerReceiver<Self::Scheduler, Self::Value>,
    X::Value: 'a,
    Y::Value: 'a,
{
    type Output<'scope> = WhenAllOperationState<'scope, X::Output<'scope>, Y::Output<'scope>>
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

struct SplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    phantom: PhantomData<fn(Sch)>,
    error: Option<Error>,
    x: Option<XValue>,
    y: Option<YValue>,
    done: bool,
    pending: u8,
    rcv: Arc<Mutex<Rcv>>,
}

impl<Rcv, Sch, XValue, YValue> From<Rcv> for SplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    fn from(rcv: Rcv) -> Self {
        Self {
            phantom: PhantomData,
            error: None,
            x: None,
            y: None,
            done: false,
            pending: 2,
            rcv: Arc::new(Mutex::new(rcv)),
        }
    }
}

impl<Rcv, Sch, XValue, YValue> SplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
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
            } else if self.x.is_some() && self.y.is_some() {
                panic!("maybe_complete should only be called in case of any error/done signal, but the only signals were value signals");
            } else {
                unreachable!();
            }
        }
    }

    fn maybe_complete_with_scheduler(&mut self, sch: Sch) {
        self.pending -= 1;

        if self.ready() {
            let mut rcv = self.rcv.lock().unwrap();
            if let Some(error) = self.error.take() {
                rcv.set_error(error);
            } else if self.done {
                rcv.set_done();
            } else if let (Some(x), Some(y)) = (self.x.take(), self.y.take()) {
                rcv.set_value(sch, (x, y).cat());
            } else {
                unreachable!();
            }
        }
    }
}

pub struct XSplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    ptr: Arc<Mutex<SplitReceiver<Rcv, Sch, XValue, YValue>>>,
}

impl<Rcv, Sch, XValue, YValue> From<Arc<Mutex<SplitReceiver<Rcv, Sch, XValue, YValue>>>>
    for XSplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    fn from(ptr: Arc<Mutex<SplitReceiver<Rcv, Sch, XValue, YValue>>>) -> Self {
        Self { ptr }
    }
}

impl<Rcv, Sch, XValue, YValue> NoSchedulerReceiver<Sch, XValue>
    for XSplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    fn set_value(&mut self, sch: Sch, value: XValue) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.x.is_some() {
            panic!("value already assigned");
        }

        let _ = r.x.insert(value);
        r.maybe_complete_with_scheduler(sch);
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

pub struct YSplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    ptr: Arc<Mutex<SplitReceiver<Rcv, Sch, XValue, YValue>>>,
}

impl<Rcv, Sch, XValue, YValue> From<Arc<Mutex<SplitReceiver<Rcv, Sch, XValue, YValue>>>>
    for YSplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    fn from(ptr: Arc<Mutex<SplitReceiver<Rcv, Sch, XValue, YValue>>>) -> Self {
        Self { ptr }
    }
}

impl<Rcv, Sch, XValue, YValue> NoSchedulerReceiver<Sch, YValue>
    for YSplitReceiver<Rcv, Sch, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    Rcv: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    fn set_value(&mut self, sch: Sch, value: YValue) {
        let mut r = self.ptr.lock().unwrap();
        r.invariant();
        if r.y.is_some() {
            panic!("value already assigned");
        }

        let _ = r.y.insert(value);
        r.maybe_complete_with_scheduler(sch);
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

pub struct Wrap<Rcv, Sch, Value>
where
    Rcv: NoSchedulerReceiver<Sch, Value>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
{
    phantom: PhantomData<fn(Sch, Value)>,
    stop_source: StopSource,
    rcv: Rcv,
}

impl<Rcv, Sch, Value> Wrap<Rcv, Sch, Value>
where
    Rcv: NoSchedulerReceiver<Sch, Value>,
    Sch: Scheduler<LocalScheduler = Sch>,
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

impl<Rcv, Sch, Value> Receiver for Wrap<Rcv, Sch, Value>
where
    Rcv: NoSchedulerReceiver<Sch, Value>,
    Sch: Scheduler<LocalScheduler = Sch>,
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

impl<Rcv, Sch, Value> ReceiverOf<Sch, Value> for Wrap<Rcv, Sch, Value>
where
    Rcv: NoSchedulerReceiver<Sch, Value>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
{
    fn set_value(mut self, sch: Sch, value: Value) {
        self.rcv.set_value(sch, value)
    }
}

pub struct SchedulerReceiver<Rcv>
where
    Rcv: Receiver,
{
    rcv: Option<Rcv>,
}

impl<Rcv> SchedulerReceiver<Rcv>
where
    Rcv: Receiver,
{
    fn new(rcv: Rcv) -> Self {
        Self { rcv: Some(rcv) }
    }
}

impl<Sch, Value, Rcv> NoSchedulerReceiver<Sch, Value> for SchedulerReceiver<Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    Rcv: ReceiverOf<Sch::LocalScheduler, Value>,
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

    fn set_value(&mut self, sch: Sch, value: Value) {
        self.rcv
            .take()
            .expect("receiverhas not yet completed")
            .set_value(sch, value);
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

pub struct WhenAllFin<Sender>
where
    Sender: NoSchedulerSenderValue,
{
    stop_source: StopSource,
    sender: Sender,
}

impl<Sender> WhenAllFin<Sender>
where
    Sender: NoSchedulerSenderValue,
{
    fn new(stop_source: StopSource, sender: Sender) -> Self {
        Self {
            stop_source,
            sender,
        }
    }
}

impl<Sender> TypedSender for WhenAllFin<Sender>
where
    Sender: NoSchedulerSenderValue,
{
    type Scheduler = Sender::Scheduler;
    type Value = Sender::Value;
}

impl<'a, ScopeImpl, StopTokenImpl, Rcv, Sender>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, Rcv> for WhenAllFin<Sender>
where
    StopTokenImpl: StopToken,
    Rcv: ReceiverOf<Sender::Scheduler, Sender::Value>,
    Sender: NoSchedulerSender<'a, ScopeImpl, SchedulerReceiver<Rcv>>,
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

        let rcv = SchedulerReceiver::new(rcv);
        self.sender.connect(scope, rcv)
    }
}

impl<BindSenderImpl, Sender> BitOr<BindSenderImpl> for WhenAllFin<Sender>
where
    BindSenderImpl: BindSender<Self>,
    Sender: NoSchedulerSenderValue,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{NoSchedulerSenderImpl, NoSchedulerSenderValue};
    use crate::errors::{new_error, ErrorForTesting};
    use crate::just::Just;
    use crate::just_done::JustDone;
    use crate::just_error::JustError;
    use crate::scheduler::ImmediateScheduler;
    use crate::scheduler::Scheduler;
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
            .fin(stop_source);
        assert_eq!((1, 2, 3, 4), sender.sync_wait().unwrap().unwrap());
    }

    #[test]
    fn it_works_with_threadpool() {
        let stop_source = StopSource::default();
        let pool = ThreadPool::with_name("it_works_with_threadpool".into(), 2);
        let sender = NoSchedulerSenderImpl::new(stop_source.clone(), pool.schedule_value((1, 2)))
            .cat(NoSchedulerSenderImpl::new(
                stop_source.clone(),
                pool.schedule_value((3, 4)),
            ))
            .fin(stop_source);
        assert_eq!((1, 2, 3, 4), sender.sync_wait_send().unwrap().unwrap());
    }

    #[test]
    fn errors_are_propagated() {
        let stop_source = StopSource::default();
        match NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(ErrorForTesting::from(
                "error",
            ))),
        )
        .cat(NoSchedulerSenderImpl::new(
            stop_source.clone(),
            Just::from((3, 4)),
        ))
        .fin(stop_source)
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
    fn done_is_propagated() {
        let stop_source = StopSource::default();
        match NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustDone::<ImmediateScheduler, (i32, i32)>::new(),
        )
        .cat(NoSchedulerSenderImpl::new(
            stop_source.clone(),
            Just::from((3, 4)),
        ))
        .fin(stop_source)
        .sync_wait()
        {
            Ok(None) => {}
            _ => {
                panic!("expected cancelation");
            }
        }
    }

    #[test]
    fn errors_are_propagated_even_when_done_signal_is_present() {
        let stop_source = StopSource::default();
        match NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustDone::<ImmediateScheduler, (i32, i32)>::new(),
        )
        .cat(NoSchedulerSenderImpl::new(
            stop_source.clone(),
            JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(ErrorForTesting::from(
                "error",
            ))),
        ))
        .fin(stop_source)
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
