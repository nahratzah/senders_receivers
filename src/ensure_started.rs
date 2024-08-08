use crate::errors::Error;
use crate::just::Just;
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::scope::ScopeWrap;
use crate::start_detached::StartDetached;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
use crate::tuple::Tuple;
use std::marker::PhantomData;
use std::ops::BitOr;
use std::sync::{Arc, Mutex};

/// Start the sender-chain, while allowing for attaching further elements.
///
/// It's useful to make sure a part of a chain starts running early,
/// or is executed regardless of wether it'll be made part of a whole chain.
///
/// Because the sender executes regardless of if it is waited on, it may not have any non-static references.
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
    TS::Output: StartDetached,
{
    let state = Arc::new(State::default());
    (ts | StateSender {
        state: state.clone(),
    })
    .start_detached();
    EnsureStartedTS::from(state)
}

pub struct EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    state: StatePtr<Sch, Value>,
}

impl<Sch, Value> From<StatePtr<Sch, Value>> for EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    fn from(state: StatePtr<Sch, Value>) -> Self {
        Self { state }
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

impl<'a, Scope, Rcv, Sch, Value> TypedSenderConnect<'a, Scope, Rcv> for EnsureStartedTS<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Scope: Clone + ScopeWrap<ImmediateScheduler, StateReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: ReceiverOf<Sch, Value>,
    Sch::Sender:
        for<'b> TypedSenderConnect<'b, Scope, crate::just::ReceiverWrapper<'b, Rcv, Sch, Value>>,
{
    type Output<'scope> = EnsureStartedOpstate<'scope, Sch, Value, Scope, Rcv>
    where
        'a: 'scope,
        Scope: 'scope,
        Rcv: 'scope;

    fn connect<'scope>(self, scope: &Scope, rcv: Rcv) -> Self::Output<'scope>
    where
        'a: 'scope,
        Scope: 'scope,
        Rcv: 'scope,
    {
        EnsureStartedOpstate {
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
    Sch::Sender:
        for<'a> TypedSenderConnect<'a, Scope, crate::just::ReceiverWrapper<'a, Rcv, Sch, Value>>,
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
    Sch::Sender:
        for<'a> TypedSenderConnect<'a, Scope, crate::just::ReceiverWrapper<'a, Rcv, Sch, Value>>,
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

struct State<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    signal: Mutex<StateEnum<Sch, Value>>,
}

impl<Sch, Value> State<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
{
    fn assign_signal(&self, new_signal: Signal<Sch, Value>) {
        let mut signal = self.signal.lock().unwrap();
        match &mut *signal {
            StateEnum::Pending => *signal = StateEnum::Signal(Some(new_signal)),
            StateEnum::Attached(opt_opstate) => {
                let opstate_fn = opt_opstate
                    .take()
                    .expect("signal is delivered for the first time");
                *signal = StateEnum::Signal(Some(new_signal));
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
        Just<'scope, Sch, Value>:
            TypedSender<Scheduler = Sch, Value = Value> + TypedSenderConnect<'scope, Scope, Rcv>,
        Rcv: 'scope + ReceiverOf<Sch, Value>,
        Scope: 'scope + ScopeWrap<ImmediateScheduler, StateReceiverWrapper<Sch, Value, Rcv>>,
    {
        let mut signal = self.signal.lock().unwrap();
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
                    .connect(&scope, rcv)
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
            signal: Mutex::new(StateEnum::Pending),
        }
    }
}

type StatePtr<Sch, Value> = Arc<State<Sch, Value>>;

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
{
    type Scheduler = TS::Scheduler;
    type Value = TS::Value;
}

impl<'a, Scope, Rcv, TS> TypedSenderConnect<'a, Scope, Rcv> for StateSenderTS<TS>
where
    TS: TypedSenderConnect<
        'a,
        Scope,
        StateReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value, Rcv>,
    >,
    TS::Value: 'static,
    Rcv: ReceiverOf<TS::Scheduler, TS::Value>,
{
    type Output<'scope> = TS::Output<'scope> where 'a: 'scope, Scope: 'scope, Rcv: 'scope;

    fn connect<'scope>(self, scope: &Scope, rcv: Rcv) -> Self::Output<'scope>
    where
        'a: 'scope,
        Scope: 'scope,
        Rcv: 'scope,
    {
        self.ts.connect(
            scope,
            StateReceiver {
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

pub struct StateReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Rcv: ReceiverOf<Sch, Value>,
{
    state: Arc<State<Sch, Value>>,
    rcv: Rcv,
}

impl<Sch, Value, Rcv> StateReceiverWrapper<Sch, Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'static + Tuple,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn new(state: Arc<State<Sch, Value>>, rcv: Rcv) -> Self {
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
        match &mut *self.state.signal.lock().unwrap() {
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
