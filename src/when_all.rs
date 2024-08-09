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
	{
            $typed_sender
	}
    };
    ($typed_sender_0:expr, $typed_sender_1:expr $(,)?) => {{
        use senders_receivers;
        senders_receivers::WhenAll::new($typed_sender_0, $typed_sender_1)
    }};
    ($typed_sender_0:expr, $typed_sender_1:expr, $($tail:expr),+ $(,)?) => {{
        use senders_receivers;
        senders_receivers::when_all!(
            senders_receivers::when_all!($typed_sender_0, $typed_sender_1),
            $($tail),+)
    }};
}

/// [TypedSender] for [when_all!].
/// This takes two senders, and completes with the concatenation of the values.
///
/// Recommend you use [when_all!], and not use this.
pub struct WhenAll<'a, Sch, X, Y>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    X: TypedSender<Scheduler = Sch>,
    Y: TypedSender<Scheduler = Sch>,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    phantom: PhantomData<&'a ()>,
    x: X,
    y: Y,
}

impl<'a, Sch, X, Y> WhenAll<'a, Sch, X, Y>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    X: TypedSender<Scheduler = Sch>,
    Y: TypedSender<Scheduler = Sch>,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    /// Create a new [WhenAll], which will combine the two senders into a single sender.
    pub fn new(x: X, y: Y) -> Self {
        Self {
            phantom: PhantomData,
            x,
            y,
        }
    }
}

impl<'a, Sch, X, Y> TypedSender for WhenAll<'a, Sch, X, Y>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    X: TypedSender<Scheduler = Sch>,
    Y: TypedSender<Scheduler = Sch>,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    type Scheduler = Sch;
    type Value = <(X::Value, Y::Value) as TupleCat>::Output;
}

impl<'a, BindSenderImpl, Sch, X, Y> BitOr<BindSenderImpl> for WhenAll<'a, Sch, X, Y>
where
    BindSenderImpl: BindSender<Self>,
    Sch: Scheduler<LocalScheduler = Sch>,
    X: TypedSender<Scheduler = Sch>,
    Y: TypedSender<Scheduler = Sch>,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType, Sch, X, Y>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType> for WhenAll<'a, Sch, X, Y>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    X: TypedSender<Scheduler = Sch>
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            StopTokenImpl,
            XReceiver<ReceiverType, <X as TypedSender>::Value, <Y as TypedSender>::Value>,
        >,
    Y: TypedSender<Scheduler = Sch>
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            StopTokenImpl,
            YReceiver<ReceiverType, <X as TypedSender>::Value, <Y as TypedSender>::Value>,
        >,
    X::Value: 'a,
    Y::Value: 'a,
    (X::Value, Y::Value): TupleCat,
    <(X::Value, Y::Value) as TupleCat>::Output: Tuple,
    ScopeImpl: Clone,
    StopTokenImpl: Clone,
    ReceiverType: ReceiverOf<Sch, <(X::Value, Y::Value) as TupleCat>::Output>,
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

    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        stop_token: StopTokenImpl,
        receiver: ReceiverType,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        let outer_receiver = WhenAllReceiver::new(receiver);
        let x_receiver = XReceiver::new(outer_receiver.clone());
        let y_receiver = YReceiver::new(outer_receiver);
        let stop_token_clone = stop_token.clone();
        WhenAllOperationState::new(
            self.x.connect(scope, stop_token_clone, x_receiver),
            self.y.connect(scope, stop_token, y_receiver),
        )
    }
}

type WhenAllReceiverPtr<NestedReceiver, XValue, YValue> =
    Arc<Mutex<WhenAllReceiver<NestedReceiver, XValue, YValue>>>;

struct WhenAllReceiver<NestedReceiver, XValue, YValue>
where
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    nested: Option<NestedReceiver>,
    x: Option<XValue>,
    y: Option<YValue>,
    error: Option<Error>,
    done: bool,
    pending: u8,
}

impl<NestedReceiver, XValue, YValue> WhenAllReceiver<NestedReceiver, XValue, YValue>
where
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    fn new(nested: NestedReceiver) -> WhenAllReceiverPtr<NestedReceiver, XValue, YValue> {
        Arc::new(Mutex::new(Self {
            nested: Some(nested),
            x: None,
            y: None,
            error: None,
            done: false,
            pending: 2,
        }))
    }

    fn invariant(&self) {
        if self.nested.is_none() {
            panic!("already completed")
        }
        if self.pending == 0 {
            panic!("should have completed")
        }
    }

    fn ready(&self) -> bool {
        self.pending == 0
    }

    fn maybe_complete<Sch>(&mut self, sch: Sch)
    where
        Sch: Scheduler<LocalScheduler = Sch>,
        NestedReceiver: ReceiverOf<Sch, <(XValue, YValue) as TupleCat>::Output>,
    {
        self.pending -= 1;

        if self.ready() {
            let nested = self
                .nested
                .take()
                .expect("should not try to complete twice");
            if let Some(error) = self.error.take() {
                nested.set_error(error);
            } else if self.done {
                nested.set_done();
            } else if let (Some(x), Some(y)) = (self.x.take(), self.y.take()) {
                nested.set_value(sch, (x, y).cat());
            } else {
                unreachable!();
            }
        }
    }

    fn maybe_complete_no_value(&mut self) {
        self.pending -= 1;

        if self.ready() {
            let nested = self
                .nested
                .take()
                .expect("should not try to complete twice");
            if let Some(error) = self.error.take() {
                nested.set_error(error);
            } else if self.done {
                nested.set_done();
            } else {
                unreachable!();
            }
        }
    }

    fn assign_error(&mut self, error: Error) {
        self.invariant();
        let _ = self.error.get_or_insert(error);
        self.maybe_complete_no_value();
        // XXX once we have stop-tokens, mark this as to-be-canceled, so the code can skip doing unnecessary work.
    }

    fn assign_done(&mut self) {
        self.invariant();
        self.done = true;
        self.maybe_complete_no_value();
        // XXX once we have stop-tokens, mark this as to-be-canceled, so the code can skip doing unnecessary work.
    }

    fn assign_x<Sch>(&mut self, sch: Sch, x: XValue)
    where
        Sch: Scheduler<LocalScheduler = Sch>,
        NestedReceiver: ReceiverOf<Sch, <(XValue, YValue) as TupleCat>::Output>,
    {
        self.invariant();
        if self.x.is_some() {
            panic!("value already assigned");
        }

        let _ = self.x.insert(x);
        self.maybe_complete(sch);
    }

    fn assign_y<Sch>(&mut self, sch: Sch, y: YValue)
    where
        Sch: Scheduler<LocalScheduler = Sch>,
        NestedReceiver: ReceiverOf<Sch, <(XValue, YValue) as TupleCat>::Output>,
    {
        self.invariant();
        if self.y.is_some() {
            panic!("value already assigned");
        }

        let _ = self.y.insert(y);
        self.maybe_complete(sch);
    }
}

pub struct XReceiver<NestedReceiver, XValue, YValue>
where
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    shared: WhenAllReceiverPtr<NestedReceiver, XValue, YValue>,
}

impl<NestedReceiver, XValue, YValue> XReceiver<NestedReceiver, XValue, YValue>
where
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    fn new(shared: WhenAllReceiverPtr<NestedReceiver, XValue, YValue>) -> Self {
        Self { shared }
    }
}

impl<NestedReceiver, XValue, YValue> Receiver for XReceiver<NestedReceiver, XValue, YValue>
where
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    fn set_error(self, error: Error) {
        self.shared.lock().unwrap().assign_error(error);
    }

    fn set_done(self) {
        self.shared.lock().unwrap().assign_done();
    }
}

impl<NestedReceiver, Sch, XValue, YValue> ReceiverOf<Sch, XValue>
    for XReceiver<NestedReceiver, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver + ReceiverOf<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    fn set_value(self, sch: Sch, value: XValue) {
        self.shared.lock().unwrap().assign_x(sch, value);
    }
}

pub struct YReceiver<NestedReceiver, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    shared: WhenAllReceiverPtr<NestedReceiver, XValue, YValue>,
}

impl<NestedReceiver, XValue, YValue> YReceiver<NestedReceiver, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    fn new(shared: WhenAllReceiverPtr<NestedReceiver, XValue, YValue>) -> Self {
        Self { shared }
    }
}

impl<NestedReceiver, XValue, YValue> Receiver for YReceiver<NestedReceiver, XValue, YValue>
where
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver,
{
    fn set_error(self, error: Error) {
        self.shared.lock().unwrap().assign_error(error);
    }

    fn set_done(self) {
        self.shared.lock().unwrap().assign_done();
    }
}

impl<NestedReceiver, Sch, XValue, YValue> ReceiverOf<Sch, YValue>
    for YReceiver<NestedReceiver, XValue, YValue>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    XValue: Tuple,
    YValue: Tuple,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
    NestedReceiver: Receiver + ReceiverOf<Sch, <(XValue, YValue) as TupleCat>::Output>,
{
    fn set_value(self, sch: Sch, value: YValue) {
        self.shared.lock().unwrap().assign_y(sch, value);
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
    use super::WhenAll;
    use crate::errors::{new_error, ErrorForTesting};
    use crate::just::Just;
    use crate::just_done::JustDone;
    use crate::just_error::JustError;
    use crate::scheduler::ImmediateScheduler;
    use crate::scheduler::Scheduler;
    use crate::sync_wait::{SyncWait, SyncWaitSend};
    use threadpool::ThreadPool;

    #[test]
    fn it_works() {
        let sender = WhenAll::new(Just::from((1, 2)), Just::from((3, 4)));
        assert_eq!((1, 2, 3, 4), sender.sync_wait().unwrap().unwrap());
    }

    #[test]
    fn it_works_with_threadpool() {
        let pool = ThreadPool::with_name("it_works_with_threadpool".into(), 2);
        let sender = WhenAll::new(pool.schedule_value((1, 2)), pool.schedule_value((3, 4)));
        assert_eq!((1, 2, 3, 4), sender.sync_wait_send().unwrap().unwrap());
    }

    #[test]
    fn errors_are_propagated() {
        match WhenAll::new(
            JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(ErrorForTesting::from(
                "error",
            ))),
            Just::from((3, 4)),
        )
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
        match WhenAll::new(
            JustDone::<ImmediateScheduler, (i32, i32)>::new(),
            Just::from((3, 4)),
        )
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
        match WhenAll::new(
            JustDone::<ImmediateScheduler, (i32, i32)>::new(),
            JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(ErrorForTesting::from(
                "error",
            ))),
        )
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
