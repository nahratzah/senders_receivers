use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use crate::tuple::{Tuple, TupleCat};
use std::marker::PhantomData;
use std::ops::BitOr;
use std::sync::{Arc, Mutex};

/// Internally used macro for WhenAllTransfer.
#[macro_export]
macro_rules! when_all_transfer_inner {
    ($typed_sender:expr $(,)?) => {{
        use senders_receivers;
        senders_receivers::WhenAllTransferInner::new($typed_sender, senders_receivers::Just::default())
    }};
    ($typed_sender_0:expr, $typed_sender_1:expr $(,)?) => {{
        use senders_receivers;
        senders_receivers::WhenAllTransferInner::new($typed_sender_0, $typed_sender_1)
    }};
    ($typed_sender_0:expr, $typed_sender_1:expr, $($tail:expr),+ $(,)?) => {{
        use senders_receivers;
        senders_receivers::when_all_transfer_inner!(
            senders_receivers::when_all_transfer_inner!($typed_sender_0, $typed_sender_1),
            $($tail),+)
    }};
}

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
#[macro_export]
macro_rules! when_all_transfer {
    ($scheduler:expr $(,)?) => {
        $scheduler.schedule()
    };
    ($scheduler:expr, $typed_sender_0:expr, $typed_sender_1:expr $(,)?) => {{
        use senders_receivers;
        senders_receivers::WhenAllTransferInner::new($typed_sender_0, $typed_sender_1)
    }};
    ($scheduler:expr, $($senders:expr),+ $(,)?) => {{
        use senders_receivers;
        senders_receivers::when_all_transfer_inner!($($senders),+)
    }};
}

trait NoSchedulerSender<ScopeImpl, ReceiverType>
{
    type Output: NoSchedulerReceiver;

    fn get_receiver(self, scope: &ScopeImpl, receiver: ReceiverType) -> (Self::Output, impl OperationState);
}

trait NoSchedulerReceiver<Value>
where
    Value: Tuple,
{
    fn set_value(&mut self, value: Value);
    fn set_error(&mut self, error: Error);
    fn set_done(&mut self);
}

struct Transferrer<Sch, Value>
where
    Sch: Scheduler,
    Value: Tuple,
{
    sch: Scheduler,
    phantom: PhantomData<fn(Value)>,
}

impl<Sch, Value> WhenAllTransferTrait<Value> for Transferrer<Sch, Value>
where
    Sch: Scheduler,
    Value: Tuple,
{
}



struct WhenAllTransfer<'a> {
}







impl TypedSenderConnect<'a, ScopeImpl, ReceiverType> for WhenAllTransfer<'a> {
    fn connect<'scope>(scope: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        TailReceiver::new(self.sch, receiver)
    }
}







struct CombiningSender<Nested, Sender> {
    nested: Nested,
    sender: Sender,
}

impl<Nested, Sender> NoSchedulerSender<Nested, Sender> for CombiningSender<Nested, Sender> {
    type Output<Value> = XReceiver<Nested::Output, Sch, Value, Sender::Value>;

    fn get_receiver<'scope, Value>(self, scope: &ScopeImpl, receiver: ReceiverType) -> (Self::Output<Value>, impl OperationState<'scope>) {
        let (nested_receiver, opstate) = self.nested.get_receiver(scope, receiver);
        let (x_receiver, y_receiver) = CombiningReceiver::new(nested_receiver).split();
        let opstate = WhenAllOperationState::new(self.sender.connect(scope, y_receiver), opstate);
        (x_receiver, opstate)
    }
}

impl<'a, ScopeImpl, ReceiverType> TypedSenderConnect<'a, ScopeImpl, ReceiverType> for WhenAllTransfer<'a> {
    fn connect<'scope>(scope: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope> {
        let (nested_receiver, opstate) = self.nested.get_receiver(scope, receiver);
        WhenAllOperationState::new(
            self.sender.connect(scope, nested_receiver),
            opstate,
        )
    }
}







struct TailSender<ScopeImpl, NestedReceiver, Sch, Value> {
    sch: Sch,
    phantom: PhantomData<fn(ScopeImpl, Value) -> NestedReceiver>,
}

impl<ScopeImpl, ReceiverType, Sch, Value> NoSchedulerSender<ScopeImpl, ReceiverType> for TailSender<ScopeImpl, ReceiverType, Sch, Value> {
    type Output = TailReceiver<ScopeImpl, ReceiverType, Sch, Value>;

    fn get_receiver<'scope>(self, scope: &ScopeImpl, receiver: ReceiverType) -> (Self::Output, impl OperationState<'scope>) {
        (TailReceiver::new(self.sch, scope.clone(), nested), NoopOperationState)
    }
}







type CombiningReceiverPtr<Nested, Sch, XValue, YValue> =
    Arc<Mutex<CombiningReceiver<Nested, Sch, XValue, YValue>>>;

struct CombiningReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    nested: Nested,
    x: Option<XValue>,
    y: Option<YValue>,
    error: Option<Error>,
    done: bool,
    pending: u8,
    phantom: PhantomData<fn(Sch)>,
}

impl<Nested, Sch, XValue, YValue> CombiningReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    fn new(nested: Nested) -> CombiningReceiverPtr<Nested, Sch, XValue, YValue> {
        Self {
            nested,
            x: None,
            y: None,
            error: None,
            done: false,
            pending: 2,
            phantom: PhantomData,
        }
    }

    fn split(ptr: CombiningReceiverPtr<Nested, Sch, XValue, YValue>) -> (XReceiver<Nested, Sch, XValue, YValue>, YReceiver<Nested, Sch, XValue, YValue>){
        let x = XReceiver::new(ptr.clone());
        let y = YReceiver::new(ptr);
        (x, y)
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

    fn maybe_complete(&mut self)
    where
        Sch: Scheduler,
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
                nested.set_value((x, y).cat());
            } else {
                unreachable!();
            }
        }
    }

    fn assign_error(&mut self, error: Error) {
        self.invariant();
        let _ = self.error.get_or_insert(error);
        self.maybe_complete();
        // XXX once we have stop-tokens, mark this as to-be-canceled, so the code can skip doing unnecessary work.
    }

    fn assign_done(&mut self) {
        self.invariant();
        self.done = true;
        self.maybe_complete();
        // XXX once we have stop-tokens, mark this as to-be-canceled, so the code can skip doing unnecessary work.
    }

    fn assign_x(&mut self, x: XValue)
    where
        Sch: Scheduler,
        NestedReceiver: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
    {
        self.invariant();
        if self.x.is_some() {
            panic!("value already assigned");
        }

        let _ = self.x.insert(x);
        self.maybe_complete(sch);
    }

    fn assign_y(&mut self, y: YValue)
    where
        NestedReceiver: NoSchedulerReceiver<<(XValue, YValue) as TupleCat>::Output>,
    {
        self.invariant();
        if self.y.is_some() {
            panic!("value already assigned");
        }

        let _ = self.y.insert(y);
        self.maybe_complete(sch);
    }
}

struct XReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    shared: CombiningReceiverPtr<Nested, Sch, XValue, YValue>,
}

impl<Nested, Sch, XValue, YValue> XReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    fn new(shared: CombiningReceiverPtr<Nested, Sch, XValue, YValue>) {
        Self { shared }
    }
}

impl<Nested, Sch, XValue, YValue> NoSchedulerReceiver<XValue>
    for XReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    fn set_value(&mut self, value: XValue) {
        self.shared.lock().unwrap().assign_x(value)
    }

    fn set_error(&mut self, error: Error) {
        self.shared.lock().unwrap().assign_error(error)
    }

    fn set_done(&mut self) {
        self.shared.lock().unwrap().assign_done(error)
    }
}

struct YReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    shared: CombiningReceiverPtr<Nested, Sch, XValue, YValue>,
}

impl<Nested, Sch, XValue, YValue> YReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    fn new(shared: CombiningReceiverPtr<Nested, Sch, XValue, YValue>) {
        Self { shared }
    }
}

impl<Nested, Sch, XValue, YValue> ReceiverOf<Sch, YValue>
    for YReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    fn set_value(self, value: YValue) {
        self.shared.lock().unwrap().assign_y(value)
    }
}

impl<Nested, Sch, XValue, YValue> Receiver
    for YReceiver<Nested, Sch, XValue, YValue>
where
    Nested: NoSchedulerReceiver<Sch, <(XValue, YValue) as TupleCat>::Output>,
    Sch: Scheduler,
    (XValue, YValue): TupleCat,
    <(XValue, YValue) as TupleCat>::Output: Tuple,
{
    fn set_error(self, error: Error) {
        self.shared.lock().unwrap().assign_error(error)
    }

    fn set_done(self) {
        self.shared.lock().unwrap().assign_done(error)
    }
}

struct TailReceiver<ScopeImpl, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch, Value>,
    Sch: Scheduler,
    Value: Tuple,
{
    scope: ScopeImpl,
    nested: Option<NestedReceiver>,
    sch: Scheduler,
    phantom: PhantomData<fn(Value)>,
}

impl<ScopeImpl, NestedReceiver, Sch, Value> TailReceiver<ScopeImpl, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch, Value>,
    Sch: Scheduler,
    Value: Tuple,
{
    fn new(sch: Scheduler, scope: ScopeImpl, nested: NestedReceiver) -> Self {
        Self {
            scope,
            sch,
            nested: Some(nested),
            phantom: PhantomData,
        }
    }
}

impl<ScopeImpl, NestedReceiver, Sch, Value> NoSchedulerReceiver<Value>
    for TailReceiver<ScopeImpl, NestedReceiver, Sch, Value>
where
    NestedReceiver: ReceiverOf<Sch, Value>,
    Sch: Scheduler,
    Value: Tuple,
{
    fn set_value(&mut self, value: Value) {
        self.sch
            .schedule_value(value)
            .connect(
                self.scope,
                self.nested.take().expect("should only complete once"),
            )
            .start()
    }

    fn set_error(&mut self, error: Error) {
        self.nested.take().unwrap().set_error(error)
    }

    fn set_done(&mut self) {
        self.nested.take().unwrap().set_done()
    }
}

struct NoopOperationState;

impl OperationState<'_> for NoopOperationState {
    fn start(self) {}
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
