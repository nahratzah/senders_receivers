use crate::errors::Error;
use crate::scheduler::Scheduler;
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
        senders_receivers::NoSchedulerSenderImpl::from($typed_sender_0)
            $(.cat(senders_receivers::NoSchedulerSenderImpl::from($senders)))*
            .schedule($scheduler)
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
    fn schedule<Sch>(self, sch: Sch) -> SchedulerTS<Sch, Self>
    where
        Self: Sized,
        Sch: Scheduler,
    {
        SchedulerTS::new(sch, self)
    }
}

/// A [TypedSenderConnect] trait, except it lacks a [Scheduler](TypedSender::Scheduler) type.
pub trait NoSchedulerSender<'a, ScopeImpl, ReceiverType>: NoSchedulerSenderValue
where
    ReceiverType: NoSchedulerReceiver<<Self as NoSchedulerSenderValue>::Value>,
{
    /// Connect a [NoSchedulerReceiver] to this sender.
    fn connect<'scope>(self, scope: &ScopeImpl, rcv: ReceiverType) -> impl OperationState<'scope>
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
    sender: TS,
}

impl<TS> From<TS> for NoSchedulerSenderImpl<TS>
where
    TS: TypedSender,
{
    fn from(sender: TS) -> Self {
        Self { sender }
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
        + TypedSenderConnect<'a, ScopeImpl, Wrap<ReceiverType, <TS as TypedSender>::Value>>,
    <TS as TypedSender>::Value: 'a,
{
    fn connect<'scope>(self, scope: &ScopeImpl, rcv: ReceiverType) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        self.sender.connect(scope, Wrap::from(rcv))
    }
}

/// Wrap a [NoSchedulerReceiver] into a regular [Receiver].
struct Wrap<Rcv, Value>
where
    Rcv: NoSchedulerReceiver<Value>,
    Value: Tuple,
{
    phantom: PhantomData<fn(Value)>,
    rcv: Rcv,
}

impl<Rcv, Value> From<Rcv> for Wrap<Rcv, Value>
where
    Rcv: NoSchedulerReceiver<Value>,
    Value: Tuple,
{
    fn from(rcv: Rcv) -> Self {
        Self {
            phantom: PhantomData,
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
        self.rcv.set_error(error)
    }

    fn set_done(mut self) {
        self.rcv.set_done()
    }
}

impl<Rcv, Sch, Value> ReceiverOf<Sch, Value> for Wrap<Rcv, Value>
where
    Sch: Scheduler,
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
    fn connect<'scope>(self, scope: &ScopeImpl, rcv: ReceiverType) -> impl OperationState<'scope>
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

struct XSplitReceiver<Rcv, XValue, YValue>
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

struct YSplitReceiver<Rcv, XValue, YValue>
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
    sender: Sender,
}

impl<Sch, Sender> SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sender: NoSchedulerSenderValue,
{
    fn new(sch: Sch, sender: Sender) -> Self {
        Self { sch, sender }
    }
}

impl<Sch, Sender> TypedSender for SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sender: NoSchedulerSenderValue,
{
    type Scheduler = Sch;
    type Value = Sender::Value;
}

impl<'a, ScopeImpl, Rcv, Sch, Sender> TypedSenderConnect<'a, ScopeImpl, Rcv>
    for SchedulerTS<Sch, Sender>
where
    Sch: Scheduler,
    Sender: NoSchedulerSenderValue + NoSchedulerSender<'a, ScopeImpl, SchedulerReceiver<Sch, Rcv>>,
    Rcv: ReceiverOf<Sch, Sender::Value>,
{
    fn connect<'scope>(self, scope: &ScopeImpl, rcv: Rcv) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        Rcv: 'scope,
    {
        self.sender
            .connect(scope, SchedulerReceiver::new(self.sch, rcv))
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

struct SchedulerReceiver<Sch, Rcv>
where
    Sch: Scheduler,
    Rcv: Receiver,
{
    sch: Option<Sch>,
    rcv: Option<Rcv>,
}

impl<Sch, Rcv> SchedulerReceiver<Sch, Rcv>
where
    Sch: Scheduler,
    Rcv: Receiver,
{
    fn new(sch: Sch, rcv: Rcv) -> Self {
        Self {
            sch: Some(sch),
            rcv: Some(rcv),
        }
    }
}

impl<Sch, Value, Rcv> NoSchedulerReceiver<Value> for SchedulerReceiver<Sch, Rcv>
where
    Sch: Scheduler,
    Value: Tuple,
    Rcv: Receiver + ReceiverOf<Sch, Value>,
{
    fn set_error(&mut self, error: Error) {
        self.rcv
            .take()
            .expect("receiver has not yet completed")
            .set_error(error)
    }

    fn set_done(&mut self) {
        self.rcv
            .take()
            .expect("receiver has not yet completed")
            .set_done()
    }

    fn set_value(&mut self, value: Value) {
        self.rcv
            .take()
            .expect("receiver has not yet completed")
            .set_value(
                self.sch.take().expect("scheduler has not been consumed"),
                value,
            )
    }
}

struct WhenAllOperationState<'scope, X, Y>
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
    use crate::sync_wait::{SyncWait, SyncWaitSend};
    use threadpool::ThreadPool;

    #[test]
    fn it_works() {
        let sender = NoSchedulerSenderImpl::from(Just::from((1, 2)))
            .cat(NoSchedulerSenderImpl::from(Just::from((3, 4))))
            .schedule(ImmediateScheduler::default());
        assert_eq!((1, 2, 3, 4), sender.sync_wait().unwrap().unwrap());
    }

    #[test]
    fn it_works_with_threadpool() {
        let pool = ThreadPool::with_name("it_works_with_threadpool".into(), 2);
        let sender = NoSchedulerSenderImpl::from(Just::from((1, 2)))
            .cat(NoSchedulerSenderImpl::from(Just::from((3, 4))))
            .schedule(pool);
        assert_eq!((1, 2, 3, 4), sender.sync_wait_send().unwrap().unwrap());
    }

    #[test]
    fn errors_are_propagated() {
        match NoSchedulerSenderImpl::from(JustError::<ImmediateScheduler, (i32, i32)>::from(
            new_error(ErrorForTesting::from("error")),
        ))
        .cat(NoSchedulerSenderImpl::from(Just::from((3, 4))))
        .schedule(ImmediateScheduler::default())
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
        match NoSchedulerSenderImpl::from(JustDone::<ImmediateScheduler, (i32, i32)>::new())
            .cat(NoSchedulerSenderImpl::from(Just::from((3, 4))))
            .schedule(ImmediateScheduler::default())
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
        match NoSchedulerSenderImpl::from(JustDone::<ImmediateScheduler, (i32, i32)>::new())
            .cat(NoSchedulerSenderImpl::from(JustError::<
                ImmediateScheduler,
                (i32, i32),
            >::from(new_error(
                ErrorForTesting::from("error"),
            ))))
            .schedule(ImmediateScheduler::default())
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
