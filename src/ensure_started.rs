use crate::errors::Error;
use crate::just::Just;
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::scope::{ScopeWrap, ScopeWrapSend};
use crate::start_detached::StartDetachedWithStopToken;
use crate::stop_token::{NeverStopToken, StopSource, StopToken, StoppableToken};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use crate::tuple::Tuple;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::BitOr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Start the sender-chain, while allowing for attaching further elements.
///
/// It's useful to make sure a part of a chain starts running early,
/// or is executed regardless of wether it'll be made part of a whole chain.
///
/// Because the sender executes regardless of if it is waited on, it may not have any non-static references.
///
/// This function does not require the sender argument to implement [Send],
/// but as a consequence the returned sender will also not implement [Send].
/// If you do need to send this across threads, or the sender uses a scheduler that does, use [ensure_started_send] instead.
///
/// The [StopToken] from the later attached operation, will be propagated into the earlier started sender chain.
///
/// Example:
/// ```
/// use senders_receivers::{ensure_started, ImmediateScheduler, Scheduler, SyncWait, Then};
/// use rand::random;
///
/// let sender = ensure_started(
///     ImmediateScheduler.schedule()
///     | Then::from(|_: ()| {
///         println!("this will be run irrespective of random below");
///         ()
///     })
/// );
///
/// if random::<bool>() {
///     let sender = sender
///     | Then::from(|_: ()| {
///         println!("this may or may not be run");
///         ()
///     });
///     sender.sync_wait();
/// }
/// ```
pub fn ensure_started<TS>(ts: TS) -> EnsureStartedTS<TS::Scheduler, TS::Value>
where
    TS: 'static + TypedSender + BitOr<StateSender<TS::Scheduler, TS::Value>>,
    TS::Value: 'static,
    TS::Output: StartDetachedWithStopToken<StoppableToken>,
{
    let stop_source = StopSource::default();
    let state = Rc::new(State::default());
    (ts | StateSender {
        state: state.clone(),
    })
    .start_detached_with_stop_token(stop_source.token());
    EnsureStartedTS::new(stop_source, state)
}

/// Start the sender-chain, while allowing for attaching further elements.
///
/// It's useful to make sure a part of a chain starts running early,
/// or is executed regardless of wether it'll be made part of a whole chain.
///
/// Because the sender executes regardless of if it is waited on, it may not have any non-static references.
///
/// This function requires the sender argument to implement [Send],
/// and the returned sender will also implement [Send].
/// [ensure_started] is the counterpart that doesn't have the [Send] requirement.
///
/// The [StopToken] from the later attached operation, will be propagated into the earlier started sender chain.
///
/// Example:
/// ```
/// use senders_receivers::{ensure_started_send, Scheduler, SyncWaitSend, Then};
/// use threadpool::ThreadPool;
/// use rand::random;
///
/// let pool = ThreadPool::with_name("example".into(), 1);
/// let sender = ensure_started_send(
///     pool.schedule()
///     | Then::from(|_: ()| {
///         println!("this will be run irrespective of random below");
///         ()
///     })
/// );
///
/// if random::<bool>() {
///     let sender = sender
///     | Then::from(|_: ()| {
///         println!("this may or may not be run");
///         ()
///     });
///     sender.sync_wait_send();
/// }
/// ```
pub fn ensure_started_send<TS>(ts: TS) -> EnsureStartedSendTS<TS::Scheduler, TS::Value>
where
    TS: 'static + TypedSender + BitOr<StateSenderSend<TS::Scheduler, TS::Value>>,
    TS::Scheduler: Send,
    TS::Value: 'static + Send,
    TS::Output: StartDetachedWithStopToken<StoppableToken>,
{
    let stop_source = StopSource::default();
    let state = Arc::new(StateSend::default());
    (ts | StateSenderSend {
        state: state.clone(),
    })
    .start_detached_with_stop_token(stop_source.token());
    EnsureStartedSendTS::new(stop_source, state)
}

pub struct EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    stop_source: StopSource,
    state: StatePtr<Sch, Value>,
}

impl<Sch, Value> EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    fn new(stop_source: StopSource, state: StatePtr<Sch, Value>) -> Self {
        Self { stop_source, state }
    }
}

impl<Sch, Value, BindSenderImpl> BitOr<BindSenderImpl> for EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<Sch, Value> TypedSender for EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    type Scheduler = Sch;
    type Value = Value;
}

impl<'a, Scope, StopTokenImpl, Rcv, Sch, Value> TypedSenderConnect<'a, Scope, StopTokenImpl, Rcv>
    for EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Scope: Clone + ScopeWrap<ImmediateScheduler, StateReceiverWrapper<Sch, Value, Rcv>>,
    StopTokenImpl: StopToken,
    Rcv: ReceiverOf<Sch, Value>,
    Sch::Sender: for<'b> TypedSenderConnect<
        'b,
        Scope,
        NeverStopToken,
        crate::just::ReceiverWrapper<'b, Rcv, Sch, Value>,
    >,
{
    type Output<'scope> = EnsureStartedOpstate<'scope, Sch, Value, Scope, Rcv>
    where
        'a: 'scope,
        Scope: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope;

    fn connect<'scope>(
        self,
        scope: &Scope,
        stop_token: StopTokenImpl,
        rcv: Rcv,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        Scope: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope,
    {
        if StopTokenImpl::STOP_POSSIBLE {
            let stop_source = self.stop_source;
            if let Err(f) = stop_token.detached_callback(move || stop_source.request_stop()) {
                // Already stopped, so just run the callback to propagate the stop-request.
                f();
            }
        }

        EnsureStartedOpstate {
            phantom: PhantomData,
            state: self.state,
            scope: scope.clone(),
            rcv,
        }
    }
}

pub struct EnsureStartedSendTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    stop_source: StopSource,
    state: StateSendPtr<Sch, Value>,
}

impl<Sch, Value> EnsureStartedSendTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    fn new(stop_source: StopSource, state: StateSendPtr<Sch, Value>) -> Self {
        Self { stop_source, state }
    }
}

impl<Sch, Value, BindSenderImpl> BitOr<BindSenderImpl> for EnsureStartedSendTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<Sch, Value> TypedSender for EnsureStartedSendTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    type Scheduler = Sch;
    type Value = Value;
}

impl<'a, Scope, StopTokenImpl, Rcv, Sch, Value> TypedSenderConnect<'a, Scope, StopTokenImpl, Rcv>
    for EnsureStartedSendTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    Scope: Clone + ScopeWrapSend<ImmediateScheduler, StateSendReceiverWrapper<Sch, Value, Rcv>>,
    StopTokenImpl: StopToken,
    Rcv: ReceiverOf<Sch, Value> + Send,
    Sch::Sender: for<'b> TypedSenderConnect<
        'b,
        Scope,
        NeverStopToken,
        crate::just::ReceiverWrapper<'b, Rcv, Sch, Value>,
    >,
{
    type Output<'scope> = EnsureStartedSendOpstate<'scope, Sch, Value, Scope, Rcv>
    where
        'a: 'scope,
        Scope: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope;

    fn connect<'scope>(
        self,
        scope: &Scope,
        stop_token: StopTokenImpl,
        rcv: Rcv,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        Scope: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope,
    {
        if StopTokenImpl::STOP_POSSIBLE {
            let stop_source = self.stop_source;
            if let Err(f) = stop_token.detached_callback(move || stop_source.request_stop()) {
                // Already stopped, so just run the callback to propagate the stop-request.
                f();
            }
        }

        EnsureStartedSendOpstate {
            phantom: PhantomData,
            state: self.state,
            scope: scope.clone(),
            rcv,
        }
    }
}

pub struct EnsureStartedOpstate<'scope, Sch, Value, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Scope: 'scope + ScopeWrap<ImmediateScheduler, StateReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: ReceiverOf<Sch, Value>,
    Sch::Sender: for<'a> TypedSenderConnect<
        'a,
        Scope,
        NeverStopToken,
        crate::just::ReceiverWrapper<'a, Rcv, Sch, Value>,
    >,
{
    phantom: PhantomData<&'scope ()>,
    state: StatePtr<Sch, Value>,
    scope: Scope,
    rcv: Rcv,
}

impl<'scope, Sch, Value, Scope, Rcv> OperationState<'scope>
    for EnsureStartedOpstate<'scope, Sch, Value, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Scope: 'scope + ScopeWrap<ImmediateScheduler, StateReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: 'scope + ReceiverOf<Sch, Value>,
    Sch::Sender: for<'a> TypedSenderConnect<
        'a,
        Scope,
        NeverStopToken,
        crate::just::ReceiverWrapper<'a, Rcv, Sch, Value>,
    >,
{
    fn start(self) {
        self.state.attach_receiver::<'scope>(self.scope, self.rcv);
    }
}

pub struct EnsureStartedSendOpstate<'scope, Sch, Value, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    Scope: 'scope + ScopeWrapSend<ImmediateScheduler, StateSendReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: ReceiverOf<Sch, Value> + Send,
    Sch::Sender: for<'a> TypedSenderConnect<
        'a,
        Scope,
        NeverStopToken,
        crate::just::ReceiverWrapper<'a, Rcv, Sch, Value>,
    >,
{
    phantom: PhantomData<&'scope ()>,
    state: StateSendPtr<Sch, Value>,
    scope: Scope,
    rcv: Rcv,
}

impl<'scope, Sch, Value, Scope, Rcv> OperationState<'scope>
    for EnsureStartedSendOpstate<'scope, Sch, Value, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    Scope: 'scope + ScopeWrapSend<ImmediateScheduler, StateSendReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: 'scope + ReceiverOf<Sch, Value> + Send,
    Sch::Sender: for<'a> TypedSenderConnect<
        'a,
        Scope,
        NeverStopToken,
        crate::just::ReceiverWrapper<'a, Rcv, Sch, Value>,
    >,
{
    fn start(self) {
        self.state.attach_receiver::<'scope>(self.scope, self.rcv);
    }
}

enum Signal<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    Value(Sch, Value),
    Error(Error),
    Done,
}

enum StateEnum<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    Pending,
    Signal(Option<Signal<Sch, Value>>),
    Attached(Option<Box<dyn FnOnce()>>),
}

enum StateSendEnum<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    Pending,
    Signal(Option<Signal<Sch, Value>>),
    Attached(Option<Box<dyn Send + FnOnce()>>),
}

struct State<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    signal: RefCell<StateEnum<Sch, Value>>,
}

impl<Sch, Value> State<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    fn assign_signal(&self, new_signal: Signal<Sch, Value>) {
        let mut signal = self.signal.borrow_mut();
        match &mut *signal {
            StateEnum::Pending => *signal = StateEnum::Signal(Some(new_signal)),
            StateEnum::Attached(opt_opstate) => {
                let opstate_fn = opt_opstate
                    .take()
                    .expect("signal is delivered for the first time");
                *signal = StateEnum::Signal(Some(new_signal));
                drop(signal); // Release the lock, so the opstate_fn can re-acquire it.

                opstate_fn();
            }
            _ => panic!("already completed"),
        };
    }

    fn assign_value(&self, sch: Sch, value: Value) {
        self.assign_signal(Signal::Value(sch, value))
    }

    fn assign_error(&self, error: Error) {
        self.assign_signal(Signal::Error(error))
    }

    fn assign_done(&self) {
        self.assign_signal(Signal::Done)
    }

    fn attach_receiver<'scope, Scope, Rcv>(self: Rc<Self>, scope: Scope, rcv: Rcv)
    where
        Just<'scope, Sch, Value>: TypedSender<Scheduler = Sch, Value = Value>
            + TypedSenderConnect<'scope, Scope, NeverStopToken, Rcv>,
        Rcv: 'scope + ReceiverOf<Sch, Value>,
        Scope: 'scope + ScopeWrap<ImmediateScheduler, StateReceiverWrapper<Sch, Value, Rcv>>,
    {
        let mut signal = self.signal.borrow_mut();
        match &mut *signal {
            StateEnum::Pending => {
                *signal = {
                    let continuation = scope.wrap(StateReceiverWrapper::new(self.clone(), rcv));
                    StateEnum::Attached(Some(Box::new(move || {
                        continuation.set_value(ImmediateScheduler, ())
                    })))
                }
            }
            StateEnum::Signal(signal) => match signal.take().expect("signal is not consumed twice")
            {
                Signal::Value(sch, value) => Just::<'scope, Sch, Value>::with_scheduler(sch, value)
                    .connect(&scope, NeverStopToken, rcv)
                    .start(),
                Signal::Error(error) => rcv.set_error(error),
                Signal::Done => rcv.set_done(),
            },
            _ => panic!("already completed"),
        }
    }
}

impl<Sch, Value> Default for State<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    fn default() -> Self {
        Self {
            signal: RefCell::new(StateEnum::Pending),
        }
    }
}

struct StateSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    signal: Mutex<StateSendEnum<Sch, Value>>,
}

impl<Sch, Value> StateSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    fn assign_signal(&self, new_signal: Signal<Sch, Value>) {
        let mut signal = self.signal.lock().unwrap();
        match &mut *signal {
            StateSendEnum::Pending => *signal = StateSendEnum::Signal(Some(new_signal)),
            StateSendEnum::Attached(opt_opstate) => {
                let opstate_fn = opt_opstate
                    .take()
                    .expect("signal is delivered for the first time");
                *signal = StateSendEnum::Signal(Some(new_signal));
                drop(signal); // Release the lock, so the opstate_fn can re-acquire it.

                opstate_fn();
            }
            _ => panic!("already completed"),
        };
    }

    fn assign_value(&self, sch: Sch, value: Value) {
        self.assign_signal(Signal::Value(sch, value))
    }

    fn assign_error(&self, error: Error) {
        self.assign_signal(Signal::Error(error))
    }

    fn assign_done(&self) {
        self.assign_signal(Signal::Done)
    }

    fn attach_receiver<'scope, Scope, Rcv>(self: Arc<Self>, scope: Scope, rcv: Rcv)
    where
        Just<'scope, Sch, Value>: TypedSender<Scheduler = Sch, Value = Value>
            + TypedSenderConnect<'scope, Scope, NeverStopToken, Rcv>,
        Rcv: 'scope + ReceiverOf<Sch, Value> + Send,
        Scope:
            'scope + ScopeWrapSend<ImmediateScheduler, StateSendReceiverWrapper<Sch, Value, Rcv>>,
    {
        let mut signal = self.signal.lock().unwrap();
        match &mut *signal {
            StateSendEnum::Pending => {
                *signal = {
                    let continuation =
                        scope.wrap_send(StateSendReceiverWrapper::new(self.clone(), rcv));
                    StateSendEnum::Attached(Some(Box::new(move || {
                        continuation.set_value(ImmediateScheduler, ())
                    })))
                }
            }
            StateSendEnum::Signal(signal) => match signal
                .take()
                .expect("signal is not consumed twice")
            {
                Signal::Value(sch, value) => Just::<'scope, Sch, Value>::with_scheduler(sch, value)
                    .connect(&scope, NeverStopToken, rcv)
                    .start(),
                Signal::Error(error) => rcv.set_error(error),
                Signal::Done => rcv.set_done(),
            },
            _ => panic!("already completed"),
        }
    }
}

impl<Sch, Value> Default for StateSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    fn default() -> Self {
        Self {
            signal: Mutex::new(StateSendEnum::Pending),
        }
    }
}

type StatePtr<Sch, Value> = Rc<State<Sch, Value>>;
type StateSendPtr<Sch, Value> = Arc<StateSend<Sch, Value>>;

pub struct StateSender<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    state: StatePtr<Sch, Value>,
}

impl<Sch, Value> Sender for StateSender<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
}

impl<TS, Sch, Value> BindSender<TS> for StateSender<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    TS: TypedSender<Scheduler = Sch, Value = Value>,
{
    type Output = StateSenderTS<TS>;

    fn bind(self, rhs: TS) -> Self::Output {
        StateSenderTS {
            ts: rhs,
            state: self.state,
        }
    }
}

pub struct StateSenderSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
    state: StateSendPtr<Sch, Value>,
}

impl<Sch, Value> Sender for StateSenderSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
{
}

impl<TS, Sch, Value> BindSender<TS> for StateSenderSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    TS: TypedSender<Scheduler = Sch, Value = Value>,
{
    type Output = StateSenderSendTS<TS>;

    fn bind(self, rhs: TS) -> Self::Output {
        StateSenderSendTS {
            ts: rhs,
            state: self.state,
        }
    }
}

pub struct StateSenderTS<TS>
where
    TS: TypedSender,
    TS::Value: 'static,
{
    ts: TS,
    state: StatePtr<TS::Scheduler, TS::Value>,
}

impl<TS> TypedSender for StateSenderTS<TS>
where
    TS: TypedSender,
    TS::Value: 'static,
{
    type Scheduler = TS::Scheduler;
    type Value = TS::Value;
}

impl<'a, Scope, StopTokenImpl, Rcv, TS> TypedSenderConnect<'a, Scope, StopTokenImpl, Rcv>
    for StateSenderTS<TS>
where
    TS: TypedSenderConnect<
        'a,
        Scope,
        StopTokenImpl,
        StateReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value, Rcv>,
    >,
    TS::Value: 'static,
    Rcv: ReceiverOf<TS::Scheduler, TS::Value>,
    StopTokenImpl: StopToken,
{
    type Output<'scope> = TS::Output<'scope>
    where
	'a: 'scope,
	Scope: 'scope,
	StopTokenImpl: 'scope,
        Rcv: 'scope;

    fn connect<'scope>(
        self,
        scope: &Scope,
        stop_token: StopTokenImpl,
        rcv: Rcv,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        Scope: 'scope,
        Rcv: 'scope,
    {
        self.ts.connect(
            scope,
            stop_token,
            StateReceiver {
                state: self.state,
                nested: rcv,
            },
        )
    }
}

pub struct StateSenderSendTS<TS>
where
    TS: TypedSender,
    TS::Scheduler: Send,
    TS::Value: 'static + Send,
{
    ts: TS,
    state: StateSendPtr<TS::Scheduler, TS::Value>,
}

impl<TS> TypedSender for StateSenderSendTS<TS>
where
    TS: TypedSender,
    TS::Scheduler: Send,
    TS::Value: 'static + Send,
{
    type Scheduler = TS::Scheduler;
    type Value = TS::Value;
}

impl<'a, Scope, StopTokenImpl, Rcv, TS> TypedSenderConnect<'a, Scope, StopTokenImpl, Rcv>
    for StateSenderSendTS<TS>
where
    TS: TypedSenderConnect<
        'a,
        Scope,
        StopTokenImpl,
        StateSendReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value, Rcv>,
    >,
    TS::Scheduler: Send,
    TS::Value: 'static + Send,
    Rcv: ReceiverOf<TS::Scheduler, TS::Value>,
    StopTokenImpl: StopToken,
{
    type Output<'scope> = TS::Output<'scope>
    where
        'a: 'scope,
        Scope: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope;

    fn connect<'scope>(
        self,
        scope: &Scope,
        stop_token: StopTokenImpl,
        rcv: Rcv,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        Scope: 'scope,
        StopTokenImpl: 'scope,
        Rcv: 'scope,
    {
        self.ts.connect(
            scope,
            stop_token,
            StateSendReceiver {
                state: self.state,
                nested: rcv,
            },
        )
    }
}

pub struct StateReceiver<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    NestedReceiver: Receiver,
{
    state: StatePtr<Sch, Value>,
    nested: NestedReceiver,
}

impl<Sch, Value, NestedReceiver> Receiver for StateReceiver<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    NestedReceiver: Receiver,
{
    fn set_error(self, error: Error) {
        self.state.assign_error(error);
        self.nested.set_done();
    }

    fn set_done(self) {
        self.state.assign_done();
        self.nested.set_done();
    }
}

impl<Sch, Value, NestedReceiver> ReceiverOf<Sch, Value>
    for StateReceiver<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    NestedReceiver: Receiver,
{
    fn set_value(self, sch: Sch, value: Value) {
        self.state.assign_value(sch, value);
        self.nested.set_done();
    }
}

pub struct StateSendReceiver<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    NestedReceiver: Receiver,
{
    state: StateSendPtr<Sch, Value>,
    nested: NestedReceiver,
}

impl<Sch, Value, NestedReceiver> Receiver for StateSendReceiver<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    NestedReceiver: Receiver,
{
    fn set_error(self, error: Error) {
        self.state.assign_error(error);
        self.nested.set_done();
    }

    fn set_done(self) {
        self.state.assign_done();
        self.nested.set_done();
    }
}

impl<Sch, Value, NestedReceiver> ReceiverOf<Sch, Value>
    for StateSendReceiver<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    NestedReceiver: Receiver,
{
    fn set_value(self, sch: Sch, value: Value) {
        self.state.assign_value(sch, value);
        self.nested.set_done();
    }
}

pub struct StateReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Rcv: ReceiverOf<Sch, Value>,
{
    state: StatePtr<Sch, Value>,
    rcv: Rcv,
}

impl<Sch, Value, Rcv> StateReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn new(state: StatePtr<Sch, Value>, rcv: Rcv) -> Self {
        Self { state, rcv }
    }
}

impl<Sch, Value, Rcv> Receiver for StateReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn set_error(self, error: Error) {
        self.rcv.set_error(error);
    }

    fn set_done(self) {
        self.rcv.set_done();
    }
}

impl<AnySch, Sch, Value, Rcv> ReceiverOf<AnySch, ()> for StateReceiverWrapper<Sch, Value, Rcv>
where
    AnySch: Scheduler<LocalScheduler = AnySch>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn set_value(self, _: AnySch, _: ()) {
        match &mut *self.state.signal.borrow_mut() {
            StateEnum::Signal(signal) => match signal.take().expect("signal has not been consumed")
            {
                Signal::Value(sch, value) => self.rcv.set_value(sch, value),
                Signal::Error(error) => self.rcv.set_error(error),
                Signal::Done => self.rcv.set_done(),
            },
            _ => panic!("expected state to have a signal"),
        }
    }
}

pub struct StateSendReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    Rcv: ReceiverOf<Sch, Value>,
{
    state: StateSendPtr<Sch, Value>,
    rcv: Rcv,
}

impl<Sch, Value, Rcv> StateSendReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn new(state: StateSendPtr<Sch, Value>, rcv: Rcv) -> Self {
        Self { state, rcv }
    }
}

impl<Sch, Value, Rcv> Receiver for StateSendReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn set_error(self, error: Error) {
        self.rcv.set_error(error);
    }

    fn set_done(self) {
        self.rcv.set_done();
    }
}

impl<AnySch, Sch, Value, Rcv> ReceiverOf<AnySch, ()> for StateSendReceiverWrapper<Sch, Value, Rcv>
where
    AnySch: Scheduler<LocalScheduler = AnySch>,
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'static + Tuple + Send,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn set_value(self, _: AnySch, _: ()) {
        match &mut *self.state.signal.lock().unwrap() {
            StateSendEnum::Signal(signal) => {
                match signal.take().expect("signal has not been consumed") {
                    Signal::Value(sch, value) => self.rcv.set_value(sch, value),
                    Signal::Error(error) => self.rcv.set_error(error),
                    Signal::Done => self.rcv.set_done(),
                }
            }
            _ => panic!("expected state to have a signal"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_started;
    use super::ensure_started_send;
    use crate::errors::{new_error, ErrorForTesting};
    use crate::just::Just;
    use crate::scheduler::{ImmediateScheduler, Scheduler};
    use crate::sync_wait::{SyncWait, SyncWaitSend};
    use crate::then::Then;
    use std::sync::mpsc;
    use threadpool::ThreadPool;

    #[test]
    fn unattached_works() {
        let (tx, rx) = mpsc::channel();
        let _ = ensure_started(
            Just::default() | Then::from(move |_: ()| tx.send(10).map_err(new_error)),
        );

        assert_eq!(10, rx.recv().unwrap(), "sender chain must run regardless");
    }

    #[test]
    fn send_unattached_works() {
        let (tx, rx) = mpsc::channel();
        let pool = ThreadPool::with_name("send_unattached_works".into(), 1);
        let _ = ensure_started_send(
            pool.schedule() | Then::from(move |_: ()| tx.send(10).map_err(new_error)),
        );

        assert_eq!(10, rx.recv().unwrap(), "sender chain must run regardless");
    }

    #[test]
    fn unattached_error_does_not_panic() {
        // We merely want to confirm this doesn't cause a panic.
        let _ = ensure_started(ImmediateScheduler.schedule_error::<()>(new_error(
            ErrorForTesting::from("this error won't cause a panic"),
        )));
    }

    #[test]
    fn send_unattached_error_does_not_panic() {
        let pool = ThreadPool::with_name("send_unattached_error_does_not_panic".into(), 1);
        // We merely want to confirm this doesn't cause a panic.
        let _ = ensure_started(pool.schedule_error::<()>(new_error(ErrorForTesting::from(
            "this error won't cause a panic",
        ))));
    }

    #[test]
    fn propagates_value() {
        let sender =
            ensure_started(ImmediateScheduler.schedule_value((10,))) | Then::from(|(x,)| (x + 1,));
        assert_eq!(
            11,
            sender
                .sync_wait()
                .expect("no error")
                .expect("no cancelation")
                .0
        );
    }

    #[test]
    fn send_propagates_value() {
        let pool = ThreadPool::with_name("send_propagates_value".into(), 1);
        let sender = ensure_started_send(pool.schedule_value((10,))) | Then::from(|(x,)| (x + 1,));
        assert_eq!(
            11,
            sender
                .sync_wait_send()
                .expect("no error")
                .expect("no cancelation")
                .0
        );
    }

    #[test]
    fn propagates_error() {
        let sender = ensure_started(
            ImmediateScheduler
                .schedule_error::<(i32,)>(new_error(ErrorForTesting::from("intentional error"))),
        ) | Then::from(|(x,)| (x + 1,));
        assert_eq!(
            ErrorForTesting::from("intentional error"),
            *sender
                .sync_wait()
                .expect_err("should error")
                .downcast::<ErrorForTesting>()
                .unwrap()
        );
    }

    #[test]
    fn send_propagates_error() {
        let pool = ThreadPool::with_name("send_propagates_error".into(), 1);
        let sender = ensure_started_send(
            pool.schedule_error::<(i32,)>(new_error(ErrorForTesting::from("intentional error"))),
        ) | Then::from(|(x,)| (x + 1,));
        assert_eq!(
            ErrorForTesting::from("intentional error"),
            *sender
                .sync_wait_send()
                .expect_err("should error")
                .downcast::<ErrorForTesting>()
                .unwrap()
        );
    }

    #[test]
    fn propagates_done() {
        let sender = ensure_started(ImmediateScheduler.schedule_done::<(i32,)>())
            | Then::from(|(x,)| (x + 1,));
        assert_eq!(None, sender.sync_wait().expect("no error"));
    }

    #[test]
    fn send_propagates_done() {
        let pool = ThreadPool::with_name("send_propagates_error".into(), 1);
        let sender = ensure_started(pool.schedule_done::<(i32,)>()) | Then::from(|(x,)| (x + 1,));
        assert_eq!(None, sender.sync_wait_send().expect("no error"));
    }
}
