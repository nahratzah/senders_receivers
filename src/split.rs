use crate::errors::new_error;
use crate::errors::Error;
use crate::scheduler::ImmediateScheduler;
use crate::scheduler::Scheduler;
use crate::scope::detached_scope;
use crate::scope::ScopeImpl;
use crate::scope::{ScopeDataSendPtr, ScopeWrap, ScopeWrapSend};
use crate::traits::BindSender;
use crate::traits::OperationState;
use crate::traits::Receiver;
use crate::traits::ReceiverOf;
use crate::traits::TypedSender;
use crate::traits::TypedSenderConnect;
use crate::tuple::Tuple;
use std::cell::RefCell;
use std::error;
use std::fmt;
use std::marker::PhantomData;
use std::ops::BitOr;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

/// Split turns a sender-chain into a re-usable sender-chain.
///
/// The wrapped typed-sender will be run once, and its value will be shared across all chains that derive from this.
/// It requires that the [Value](crate::traits::TypedSender::Value) implements [Clone].
///
/// [Split] is not sharable across thread boundaries (it does not implement [Send]).
/// It therefore also doesn't require the wrapped sender-chain, or any attached sender-chains to be able to cross thread boundaries.
/// Should you require that, you'll need to use [SplitSend].
///
/// # Example: Re-using a Computation
/// ```
/// use senders_receivers::{Just, Then, Split, SyncWait};
///
/// let expensive_computation = Just::default() | Then::from(|_| (String::from("super duper expensive"),));
/// let sharable_expensive_computation = Split::from(expensive_computation); // Won't run until needed.
///
/// // Use the computation (this starts it).
/// let _ = (sharable_expensive_computation.clone() | Then::from(|(s,)| println!("The outcome is {}.", s))).sync_wait();
///
/// // Use the same computation a second time (this re-uses the outcome from before).
/// let _ = (sharable_expensive_computation | Then::from(|(s,)| println!("The outcome was {}.", s))).sync_wait();
/// ```
///
/// # Error and Done Handling
/// If the wrapped sender-chain produces an error, or the done signal, that too will be forwarded to all further sender-chains.
/// However, errors are wrapped inside a [SharedError], which sadly means that casting to the underlying error type becomes difficult.
pub struct Split<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Value: 'a + Clone,
{
    opstate: Rc<OpStateWrapper<'a, TS::Output<'a>>>,
    state: Rc<RefCell<CompletionWithCallbacks<TS::Scheduler, TS::Value>>>,
}

impl<'a, TS> From<TS> for Split<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Value: 'a + Clone,
{
    fn from(sender: TS) -> Self {
        let state = Rc::new(RefCell::new(CompletionWithCallbacks::default()));
        let opstate = Rc::new(OpStateWrapper::from({
            let state = state.clone();
            detached_scope(move |scope: &ScopeImpl<ScopeDataSendPtr>| {
                sender.connect(scope, CompletionReceiver::new(state))
            })
        }));

        Self { opstate, state }
    }
}

impl<'a, TS> Clone for Split<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Value: 'a + Clone,
{
    fn clone(&self) -> Self {
        Self {
            opstate: self.opstate.clone(),
            state: self.state.clone(),
        }
    }
}

impl<'a, TS> TypedSender for Split<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Value: 'a + Clone,
{
    type Scheduler = TS::Scheduler;
    type Value = TS::Value;
}

impl<'a, TS, Scope, Rcv> TypedSenderConnect<'a, Scope, Rcv> for Split<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Value: 'a + Clone,
    <TS::Scheduler as Scheduler>::Sender:
        for<'b> TypedSenderConnect<'b, Scope, WrapValue<TS::Value, Rcv>>,
    Scope: Clone + ScopeWrap<ImmediateScheduler, ReceiverWrapper<TS::Scheduler, TS::Value, Rcv>>,
    Rcv: ReceiverOf<TS::Scheduler, TS::Value>,
{
    type Output<'scope> = SplitOpState<'scope, 'a, TS::Scheduler, TS::Value, TS::Output<'a>, Scope, Rcv>
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
        SplitOpState::new(self.opstate, self.state, scope.clone(), rcv)
    }
}

/// Split turns a sender-chain into a re-usable sender-chain.
///
/// The wrapped typed-sender will be run once, and its value will be shared across all chains that derive from this.
/// It requires that the [Value](crate::traits::TypedSender::Value) implements [Clone].
///
/// Unlike [Split], [SplitSend] allows the wrapped sender-chain to cross thread boundaries.
/// It therefore also requires that both the wrapped sender-chain, and all attached sender-chains, are able to cross thread boundaries (by implementing [Send]).
///
/// # Example: Re-using a Computation
/// ```
/// use threadpool::ThreadPool;
/// use senders_receivers::{Just, Then, SplitSend, StartDetached, SyncWaitSend, Scheduler};
///
/// let pool = ThreadPool::with_name("example".into(), 2);
/// let expensive_computation = pool.schedule() | Then::from(|_| (String::from("super duper expensive"),));
/// let sharable_expensive_computation = SplitSend::from(expensive_computation); // Won't run until needed.
///
/// // We don't have to, but we can start the computation early.
/// // Although keep in mind that `start_detached` will panic, if the computation yields an error.
/// sharable_expensive_computation.clone().start_detached();
///
/// // Use the computation.
/// let _ = (sharable_expensive_computation.clone() | Then::from(|(s,)| println!("The outcome is {}.", s))).sync_wait_send();
///
/// // Use the same computation a second time (this re-uses the outcome from before).
/// let _ = (sharable_expensive_computation | Then::from(|(s,)| println!("The outcome was {}.", s))).sync_wait_send();
/// ```
///
/// # Error and Done Handling
/// If the wrapped sender-chain produces an error, or the done signal, that too will be forwarded to all further sender-chains.
/// However, errors are wrapped inside a [SharedError], which sadly means that casting to the underlying error type becomes difficult.
pub struct SplitSend<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiverSend<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Scheduler: Send,
    TS::Value: 'a + Clone + Send,
    TS::Output<'a>: Send,
{
    opstate: Arc<OpStateWrapperSend<'a, TS::Output<'a>>>,
    state: Arc<Mutex<CompletionWithCallbacksSend<TS::Scheduler, TS::Value>>>,
}

impl<'a, TS> From<TS> for SplitSend<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiverSend<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Scheduler: Send,
    TS::Value: 'a + Clone + Send,
    TS::Output<'a>: Send,
{
    fn from(sender: TS) -> Self {
        let state = Arc::new(Mutex::new(CompletionWithCallbacksSend::default()));
        let opstate = Arc::new(OpStateWrapperSend::from({
            let state = state.clone();
            detached_scope(move |scope: &ScopeImpl<ScopeDataSendPtr>| {
                sender.connect(scope, CompletionReceiverSend::new(state))
            })
        }));

        Self { opstate, state }
    }
}

impl<'a, TS> Clone for SplitSend<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiverSend<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Scheduler: Send,
    TS::Value: 'a + Clone + Send,
    TS::Output<'a>: Send,
{
    fn clone(&self) -> Self {
        Self {
            opstate: self.opstate.clone(),
            state: self.state.clone(),
        }
    }
}

impl<'a, TS> TypedSender for SplitSend<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiverSend<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Scheduler: Send,
    TS::Value: Clone + Send,
    TS::Output<'a>: Send,
{
    type Scheduler = TS::Scheduler;
    type Value = TS::Value;
}

impl<'a, TS, Scope, Rcv> TypedSenderConnect<'a, Scope, Rcv> for SplitSend<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiverSend<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Scheduler: Send,
    TS::Value: 'a + Clone + Send,
    for<'scope> TS::Output<'scope>: Send,
    <TS::Scheduler as Scheduler>::Sender:
        for<'b> TypedSenderConnect<'b, Scope, WrapValue<TS::Value, Rcv>>,
    Scope: Clone
        + ScopeWrapSend<ImmediateScheduler, ReceiverWrapperSend<TS::Scheduler, TS::Value, Rcv>>
        + Send,
    Scope::WrapSendOutput: Send,
    Rcv: ReceiverOf<TS::Scheduler, TS::Value> + Send,
{
    type Output<'scope> = SplitOpStateSend<'scope, 'a, TS::Scheduler, TS::Value, TS::Output<'a>, Scope, Rcv>
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
        SplitOpStateSend::new(self.opstate, self.state, scope.clone(), rcv)
    }
}

struct CompletionWithCallbacks<Sch, Value>
where
    Sch: Scheduler,
    Value: Tuple + Clone,
{
    completion: Completion<Sch, Value>,
    receivers: Vec<Box<dyn FnOnce()>>,
}

impl<Sch, Value> CompletionWithCallbacks<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
{
    fn assign_value(&mut self, sch: Sch, value: Value) -> Vec<Box<dyn FnOnce()>> {
        match self.completion {
            Completion::Pending => {}
            _ => panic!("expected to be in the running state"),
        }
        self.completion = Completion::Value(sch, value);

        self.receivers.split_off(0)
    }

    fn assign_error(&mut self, error: Error) -> Vec<Box<dyn FnOnce()>> {
        match self.completion {
            Completion::Pending => {}
            _ => panic!("expected to be in the running state"),
        }
        self.completion = Completion::Error(SharedError::from(error));

        self.receivers.split_off(0)
    }

    fn assign_done(&mut self) -> Vec<Box<dyn FnOnce()>> {
        match self.completion {
            Completion::Pending => {}
            _ => panic!("expected to be in the running state"),
        }
        self.completion = Completion::Done;

        self.receivers.split_off(0)
    }
}

impl<Sch, Value> Default for CompletionWithCallbacks<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
{
    fn default() -> Self {
        Self {
            completion: Completion::Pending,
            receivers: Vec::new(),
        }
    }
}

struct CompletionWithCallbacksSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
{
    completion: Completion<Sch, Value>,
    receivers: Vec<Box<dyn Send + FnOnce()>>,
}

impl<Sch, Value> CompletionWithCallbacksSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
{
    fn assign_value(&mut self, sch: Sch, value: Value) -> Vec<Box<dyn Send + FnOnce()>> {
        match self.completion {
            Completion::Pending => {}
            _ => panic!("expected to be in the running state"),
        }
        self.completion = Completion::Value(sch, value);

        self.receivers.split_off(0)
    }

    fn assign_error(&mut self, error: Error) -> Vec<Box<dyn Send + FnOnce()>> {
        match self.completion {
            Completion::Pending => {}
            _ => panic!("expected to be in the running state"),
        }
        self.completion = Completion::Error(SharedError::from(error));

        self.receivers.split_off(0)
    }

    fn assign_done(&mut self) -> Vec<Box<dyn Send + FnOnce()>> {
        match self.completion {
            Completion::Pending => {}
            _ => panic!("expected to be in the running state"),
        }
        self.completion = Completion::Done;

        self.receivers.split_off(0)
    }
}

impl<Sch, Value> Default for CompletionWithCallbacksSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
{
    fn default() -> Self {
        Self {
            completion: Completion::Pending,
            receivers: Vec::new(),
        }
    }
}

enum Completion<Sch, Value>
where
    Sch: Scheduler,
    Value: Tuple + Clone,
{
    Pending,
    Value(Sch, Value),
    Error(SharedError),
    Done,
}

pub struct CompletionReceiver<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
{
    state: Rc<RefCell<CompletionWithCallbacks<Sch, Value>>>,
}

impl<Sch, Value> CompletionReceiver<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
{
    fn new(state: Rc<RefCell<CompletionWithCallbacks<Sch, Value>>>) -> Self {
        Self { state }
    }
}

impl<Sch, Value> Receiver for CompletionReceiver<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
{
    fn set_error(self, error: Error) {
        let mut ops = (*self.state).borrow_mut().assign_error(error);
        for i in ops.drain(0..) {
            i();
        }
    }

    fn set_done(self) {
        let mut ops = (*self.state).borrow_mut().assign_done();
        for i in ops.drain(0..) {
            i();
        }
    }
}

impl<Sch, Value> ReceiverOf<Sch, Value> for CompletionReceiver<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
{
    fn set_value(self, sch: Sch, value: Value) {
        let mut ops = (*self.state).borrow_mut().assign_value(sch, value);
        for i in ops.drain(0..) {
            i();
        }
    }
}

pub struct CompletionReceiverSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
{
    state: Arc<Mutex<CompletionWithCallbacksSend<Sch, Value>>>,
}

impl<Sch, Value> CompletionReceiverSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
{
    fn new(state: Arc<Mutex<CompletionWithCallbacksSend<Sch, Value>>>) -> Self {
        Self { state }
    }
}

impl<Sch, Value> Receiver for CompletionReceiverSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
{
    fn set_error(self, error: Error) {
        let mut ops = self.state.lock().unwrap().assign_error(error);
        for i in ops.drain(0..) {
            i();
        }
    }

    fn set_done(self) {
        let mut ops = self.state.lock().unwrap().assign_done();
        for i in ops.drain(0..) {
            i();
        }
    }
}

impl<Sch, Value> ReceiverOf<Sch, Value> for CompletionReceiverSend<Sch, Value>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
{
    fn set_value(self, sch: Sch, value: Value) {
        let mut ops = self.state.lock().unwrap().assign_value(sch, value);
        for i in ops.drain(0..) {
            i();
        }
    }
}

pub struct WrapValue<Value, Rcv>
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
        self.rcv.set_error(error)
    }

    fn set_done(self) {
        self.rcv.set_done()
    }
}

impl<Sch, Value, Rcv> ReceiverOf<Sch, ()> for WrapValue<Value, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    Rcv: Receiver + ReceiverOf<Sch, Value>,
{
    fn set_value(self, sch: Sch, _: ()) {
        self.rcv.set_value(sch, self.value)
    }
}

/// Wrap an [Error], so it can be shared across multiple sender-chains.
///
/// This is used by [Split] and [SplitSend] to share errors between multiple derived sender-chains.
///
/// My alternatives were to enforce that all errors implement [Clone], or to use [a shared pointer](Arc).
/// I chose the latter, because I don't think all errors in the standard library implement [Clone],
/// and it would probably be difficult for other types too.
#[derive(Clone)]
pub struct SharedError {
    ptr: Arc<Error>,
}

impl SharedError {
    /// Get the underlying error.
    ///
    /// This is a shared pointer, because... well, it's a shared error.
    pub fn get_ptr(&self) -> Arc<Error> {
        return self.ptr.clone();
    }
}

impl From<Error> for SharedError {
    fn from(error: Error) -> Self {
        Self {
            ptr: Arc::new(error),
        }
    }
}

impl fmt::Debug for SharedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.ptr, f)
    }
}

impl fmt::Display for SharedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.ptr, f)
    }
}

impl error::Error for SharedError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.ptr.source()
    }

    // This is deprecated, so I won't implement it, because I want to avoid compiler warnings.
    //fn description(&self) -> &str {
    //    self.ptr.description()
    //}

    // This is deprecated, so I won't implement it, because I want to avoid compiler warnings.
    //fn cause(&self) -> Option<&dyn error::Error> {
    //    self.ptr.cause()
    //}

    // Requires nightly.
    //#[unstable(feature = "error_generic_member_access", issue = "99301")]
    //fn provide<'a>(&'a self, request: &mut error::Request<'a>) {
    //    self.ptr.provide(request)
    //}
}

pub struct SplitOpState<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'scope + Clone + Tuple,
    Sch::Sender: for<'b> TypedSenderConnect<'b, Scope, WrapValue<Value, Rcv>>,
    Scope: ScopeWrap<ImmediateScheduler, ReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: 'scope + ReceiverOf<Sch, Value>,
    OpState: OperationState<'a>,
{
    phantom: PhantomData<&'scope ()>,
    opstate: Rc<OpStateWrapper<'a, OpState>>,
    state: Rc<RefCell<CompletionWithCallbacks<Sch, Value>>>,
    scope: Scope,
    rcv: Rcv,
}

impl<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
    SplitOpState<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'scope + Clone + Tuple,
    Sch::Sender: for<'b> TypedSenderConnect<'b, Scope, WrapValue<Value, Rcv>>,
    Scope: ScopeWrap<ImmediateScheduler, ReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: 'scope + ReceiverOf<Sch, Value>,
    OpState: OperationState<'a>,
{
    fn new(
        opstate: Rc<OpStateWrapper<'a, OpState>>,
        state: Rc<RefCell<CompletionWithCallbacks<Sch, Value>>>,
        scope: Scope,
        rcv: Rcv,
    ) -> Self {
        Self {
            phantom: PhantomData,
            opstate,
            state,
            scope,
            rcv,
        }
    }
}

impl<'scope, 'a, Sch, Value, OpState, Scope, Rcv> OperationState<'scope>
    for SplitOpState<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: 'scope + Clone + Tuple,
    Sch::Sender: for<'b> TypedSenderConnect<'b, Scope, WrapValue<Value, Rcv>>,
    Scope: ScopeWrap<ImmediateScheduler, ReceiverWrapper<Sch, Value, Rcv>>,
    Rcv: 'scope + ReceiverOf<Sch, Value>,
    OpState: OperationState<'a>,
{
    fn start(self) {
        let mut state = (*self.state).borrow_mut();
        match &state.completion {
            Completion::Value(sch, value) => sch
                .schedule()
                .connect(&self.scope, WrapValue::new(value.clone(), self.rcv))
                .start(),
            Completion::Error(error) => self.rcv.set_error(new_error(error.clone())),
            Completion::Done => self.rcv.set_done(),
            Completion::Pending => {
                let receiver = ReceiverWrapper::new(self.state.clone(), self.rcv);
                state
                    .receivers
                    .push(Box::new(receiver.into_fn(&self.scope)));

                // Unlock state: if the sender-chain completes on this thread, we want to avoid dead-lock.
                drop(state);

                // We must make sure `ensure_started()` is the last operation.
                // Because if the sender-chain is started before the value is assigned:
                // - the opstate might deadlock trying to complete (if we held the mutex).
                // - if we didn't hold the mutex, it would invalidate our match branch.
                // - if the `state.receivers.push` operation fails, we would break the rule that scope must outlive the operation.
                self.opstate.ensure_started();
            }
        }
    }
}

pub struct SplitOpStateSend<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'scope + Clone + Tuple + Send,
    Sch::Sender: for<'b> TypedSenderConnect<'b, Scope, WrapValue<Value, Rcv>>,
    Scope: 'scope + ScopeWrapSend<ImmediateScheduler, ReceiverWrapperSend<Sch, Value, Rcv>> + Send,
    Scope::WrapSendOutput: Send,
    Rcv: 'scope + ReceiverOf<Sch, Value> + Send,
    OpState: OperationState<'a> + Send,
{
    phantom: PhantomData<&'scope ()>,
    opstate: Arc<OpStateWrapperSend<'a, OpState>>,
    state: Arc<Mutex<CompletionWithCallbacksSend<Sch, Value>>>,
    scope: Scope,
    rcv: Rcv,
}

impl<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
    SplitOpStateSend<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'scope + Clone + Tuple + Send,
    Sch::Sender: for<'b> TypedSenderConnect<'b, Scope, WrapValue<Value, Rcv>>,
    Scope: 'scope + ScopeWrapSend<ImmediateScheduler, ReceiverWrapperSend<Sch, Value, Rcv>> + Send,
    Scope::WrapSendOutput: Send,
    Rcv: 'scope + ReceiverOf<Sch, Value> + Send,
    OpState: OperationState<'a> + Send,
{
    fn new(
        opstate: Arc<OpStateWrapperSend<'a, OpState>>,
        state: Arc<Mutex<CompletionWithCallbacksSend<Sch, Value>>>,
        scope: Scope,
        rcv: Rcv,
    ) -> Self {
        Self {
            phantom: PhantomData,
            opstate,
            state,
            scope,
            rcv,
        }
    }
}

impl<'scope, 'a, Sch, Value, OpState, Scope, Rcv> OperationState<'scope>
    for SplitOpStateSend<'scope, 'a, Sch, Value, OpState, Scope, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: 'scope + Clone + Tuple + Send,
    Sch::Sender: for<'b> TypedSenderConnect<'b, Scope, WrapValue<Value, Rcv>>,
    Scope: 'scope + ScopeWrapSend<ImmediateScheduler, ReceiverWrapperSend<Sch, Value, Rcv>> + Send,
    Scope::WrapSendOutput: Send,
    Rcv: 'scope + ReceiverOf<Sch, Value> + Send,
    OpState: OperationState<'a> + Send,
{
    fn start(self) {
        let mut state = self.state.lock().unwrap();
        match &state.completion {
            Completion::Value(sch, value) => sch
                .schedule()
                .connect(&self.scope, WrapValue::new(value.clone(), self.rcv))
                .start(),
            Completion::Error(error) => self.rcv.set_error(new_error(error.clone())),
            Completion::Done => self.rcv.set_done(),
            Completion::Pending => {
                let receiver = ReceiverWrapperSend::new(self.state.clone(), self.rcv);
                state
                    .receivers
                    .push(Box::new(receiver.into_fn(&self.scope)));

                // Unlock state: if the sender-chain completes on this thread, we want to avoid dead-lock.
                drop(state);

                // We must make sure `ensure_started()` is the last operation.
                // Because if the sender-chain is started before the value is assigned:
                // - the opstate might deadlock trying to complete (if we held the mutex).
                // - if we didn't hold the mutex, it would invalidate our match branch.
                // - if the `state.receivers.push` operation fails, we would break the rule that scope must outlive the operation.
                self.opstate.ensure_started();
            }
        };
    }
}

struct OpStateWrapper<'scope, OpState>
where
    OpState: OperationState<'scope>,
{
    phantom: PhantomData<&'scope ()>,
    opstate: RefCell<Option<OpState>>,
}

impl<'scope, OpState> OpStateWrapper<'scope, OpState>
where
    OpState: OperationState<'scope>,
{
    fn ensure_started(&self) {
        if let Some(opstate) = self.opstate.borrow_mut().take() {
            opstate.start()
        }
    }
}

impl<'scope, OpState> From<OpState> for OpStateWrapper<'scope, OpState>
where
    OpState: OperationState<'scope>,
{
    fn from(opstate: OpState) -> Self {
        Self {
            phantom: PhantomData,
            opstate: RefCell::new(Some(opstate)),
        }
    }
}

struct OpStateWrapperSend<'scope, OpState>
where
    OpState: OperationState<'scope> + Send,
{
    phantom: PhantomData<&'scope ()>,
    opstate: Mutex<Option<OpState>>,
}

impl<'scope, OpState> OpStateWrapperSend<'scope, OpState>
where
    OpState: OperationState<'scope> + Send,
{
    fn ensure_started(&self) {
        if let Some(opstate) = self.opstate.lock().unwrap().take() {
            opstate.start()
        }
    }
}

impl<'scope, OpState> From<OpState> for OpStateWrapperSend<'scope, OpState>
where
    OpState: OperationState<'scope> + Send,
{
    fn from(opstate: OpState) -> Self {
        Self {
            phantom: PhantomData,
            opstate: Mutex::new(Some(opstate)),
        }
    }
}

/// Take the [CompletionWithCallbacks] state and populate the receiver with it.
/// Ensures the receiver is scheduled using the same scheduler.
pub struct ReceiverWrapper<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
    NestedReceiver: ReceiverOf<Sch, Value>,
{
    state: Rc<RefCell<CompletionWithCallbacks<Sch, Value>>>,
    nested: NestedReceiver,
}

impl<Sch, Value, NestedReceiver> ReceiverWrapper<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
    NestedReceiver: ReceiverOf<Sch, Value>,
{
    fn new(
        state: Rc<RefCell<CompletionWithCallbacks<Sch, Value>>>,
        nested: NestedReceiver,
    ) -> Self {
        Self { state, nested }
    }

    fn invoke(self) {
        match &self.state.borrow_mut().completion {
            Completion::Value(sch, value) => self.nested.set_value(sch.clone(), value.clone()), // This is fine: we run on the same scheduler.
            Completion::Error(error) => self.nested.set_error(new_error(error.clone())),
            Completion::Done => self.nested.set_done(),
            Completion::Pending => {
                panic!("ReceiverWrapper was invoked without a signal having been installed")
            }
        }
    }

    fn into_fn<Scope>(self, scope: &Scope) -> Box<dyn 'static + FnOnce()>
    where
        Scope: ScopeWrap<ImmediateScheduler, Self>,
    {
        let r = scope.wrap(self);
        Box::new(move || r.set_value(ImmediateScheduler::default(), ()))
    }
}

impl<Sch, Value, NestedReceiver> Receiver for ReceiverWrapper<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
    NestedReceiver: ReceiverOf<Sch, Value>,
{
    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }

    fn set_done(self) {
        self.nested.set_done();
    }
}

impl<AnySch, Sch, Value, NestedReceiver> ReceiverOf<AnySch, ()>
    for ReceiverWrapper<Sch, Value, NestedReceiver>
where
    AnySch: Scheduler<LocalScheduler = AnySch>,
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple + Clone,
    NestedReceiver: ReceiverOf<Sch, Value>,
{
    fn set_value(self, _: AnySch, _: ()) {
        self.invoke();
    }
}

/// Take the [CompletionWithCallbacks] state and populate the receiver with it.
/// Ensures the receiver is scheduled using the same scheduler.
pub struct ReceiverWrapperSend<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
    NestedReceiver: ReceiverOf<Sch, Value> + Send,
{
    state: Arc<Mutex<CompletionWithCallbacksSend<Sch, Value>>>,
    nested: NestedReceiver,
}

impl<Sch, Value, NestedReceiver> ReceiverWrapperSend<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
    NestedReceiver: ReceiverOf<Sch, Value> + Send,
{
    fn new(
        state: Arc<Mutex<CompletionWithCallbacksSend<Sch, Value>>>,
        nested: NestedReceiver,
    ) -> Self {
        Self { state, nested }
    }

    fn invoke(self) {
        match &self.state.lock().unwrap().completion {
            Completion::Value(sch, value) => self.nested.set_value(sch.clone(), value.clone()), // This is fine: we run on the same scheduler.
            Completion::Error(error) => self.nested.set_error(new_error(error.clone())),
            Completion::Done => self.nested.set_done(),
            Completion::Pending => {
                panic!("ReceiverWrapperSend was invoked without a signal having been installed")
            }
        }
    }

    fn into_fn<Scope>(self, scope: &Scope) -> Box<dyn Send + FnOnce()>
    where
        Scope: ScopeWrapSend<ImmediateScheduler, Self>,
    {
        let r = scope.wrap_send(self);
        Box::new(move || r.set_value(ImmediateScheduler::default(), ()))
    }
}

impl<Sch, Value, NestedReceiver> Receiver for ReceiverWrapperSend<Sch, Value, NestedReceiver>
where
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
    NestedReceiver: ReceiverOf<Sch, Value> + Send,
{
    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }

    fn set_done(self) {
        self.nested.set_done();
    }
}

impl<AnySch, Sch, Value, NestedReceiver> ReceiverOf<AnySch, ()>
    for ReceiverWrapperSend<Sch, Value, NestedReceiver>
where
    AnySch: Scheduler<LocalScheduler = AnySch>,
    Sch: Scheduler<LocalScheduler = Sch> + Send,
    Value: Tuple + Clone + Send,
    NestedReceiver: ReceiverOf<Sch, Value> + Send,
{
    fn set_value(self, _: AnySch, _: ()) {
        self.invoke();
    }
}

impl<'a, TS, BindSenderImpl> BitOr<BindSenderImpl> for Split<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiver<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Value: 'a + Clone,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<'a, TS, BindSenderImpl> BitOr<BindSenderImpl> for SplitSend<'a, TS>
where
    TS: TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        CompletionReceiverSend<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
    >,
    TS::Scheduler: Send,
    TS::Value: 'a + Clone + Send,
    TS::Output<'a>: Send,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{Split, SplitSend};
    use crate::errors::{new_error, ErrorForTesting, Result};
    use crate::just::Just;
    use crate::let_value::LetValue;
    use crate::refs;
    use crate::scheduler::ImmediateScheduler;
    use crate::scheduler::Scheduler;
    use crate::sync_wait::{SyncWait, SyncWaitSend};
    use crate::then::Then;
    use std::sync::{Arc, Mutex};

    #[test]
    fn it_works() {
        let invocations = Mutex::new(0_i32);
        let sender = Split::from(
            Just::from(())
                | Then::from(|_| {
                    let mut guarded_invocations = invocations.lock().unwrap();
                    assert_eq!(0, *guarded_invocations);
                    *guarded_invocations += 1;
                    (*guarded_invocations,)
                }),
        );
        assert_eq!(0, *invocations.lock().unwrap(), "Split mustn't run yet.");

        let s1 = sender.clone() | Then::from(|(invocations,)| (invocations, invocations));
        let s2 = sender | Then::from(|(invocations,)| (invocations, invocations));

        assert_eq!(
            (1, 1),
            s1.sync_wait().expect("no error").expect("a value"),
            "it returns the value"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split ran its sender-chain once."
        );

        assert_eq!(
            (1, 1),
            s2.sync_wait()
                .expect("no error")
                .expect("produces the same value a second time"),
            "it returns the same value as second time"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split didn't run its sender-chain a second time."
        );
    }

    #[test]
    fn it_propagates_errors() {
        let invocations = Mutex::new(0_i32);
        let sender = Split::from(
            Just::default()
                | Then::from(|_| -> Result<(i32,)> {
                    let mut guarded_invocations = invocations.lock().unwrap();
                    assert_eq!(0, *guarded_invocations);
                    *guarded_invocations += 1;
                    Err(new_error(ErrorForTesting::from("bla bla chocoladevla")))
                }),
        );
        assert_eq!(0, *invocations.lock().unwrap(), "Split mustn't run yet.");

        let s1 = sender.clone() | Then::from(|(invocations,)| (invocations, invocations));
        let s2 = sender | Then::from(|(invocations,)| (invocations, invocations));

        s1.sync_wait().expect_err("an error should be yielded");
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split ran its sender-chain once."
        );

        s2.sync_wait()
            .expect_err("an error should be yielded the second time");
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split didn't run its sender-chain a second time."
        );
    }

    #[test]
    fn it_propagates_done() {
        let invocations = Mutex::new(0_i32);
        let sender = Split::from(
            Just::default()
                | LetValue::from(|sch: ImmediateScheduler, _| {
                    let mut guarded_invocations = invocations.lock().unwrap();
                    assert_eq!(0, *guarded_invocations);
                    *guarded_invocations += 1;
                    sch.schedule_done::<(i32,)>()
                }),
        );
        assert_eq!(0, *invocations.lock().unwrap(), "Split mustn't run yet.");

        let s1 = sender.clone() | Then::from(|(invocations,)| (invocations, invocations));
        let s2 = sender | Then::from(|(invocations,)| (invocations, invocations));

        assert_eq!(
            None,
            s1.sync_wait().expect("no error"),
            "expect done signal"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split ran its sender-chain once."
        );

        assert_eq!(
            None,
            s2.sync_wait().expect("no error"),
            "expect done signal the second time"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split didn't run its sender-chain a second time."
        );
    }

    #[test]
    fn it_works_send() {
        let invocations = Arc::new(Mutex::new(0_i32)); // XXX shouldn't require Arc here, but somehow it does?
        let sender = SplitSend::from(
            Just::from(())
                | Then::from({
                    let invocations = invocations.clone();
                    move |_| {
                        let mut guarded_invocations = invocations.lock().unwrap();
                        assert_eq!(0, *guarded_invocations);
                        *guarded_invocations += 1;
                        (*guarded_invocations,)
                    }
                }),
        );
        assert_eq!(0, *invocations.lock().unwrap(), "Split mustn't run yet.");

        let s1 = sender.clone() | Then::from(|(invocations,)| (invocations, invocations));
        let s2 = sender | Then::from(|(invocations,)| (invocations, invocations));

        assert_eq!(
            (1, 1),
            s1.sync_wait_send().expect("no error").expect("a value"),
            "it returns the value"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split ran its sender-chain once."
        );

        assert_eq!(
            (1, 1),
            s2.sync_wait_send()
                .expect("no error")
                .expect("produces the same value a second time"),
            "it returns the same value as second time"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split didn't run its sender-chain a second time."
        );
    }

    #[test]
    fn it_propagates_errors_send() {
        let invocations = Arc::new(Mutex::new(0_i32)); // XXX shouldn't require Arc here, but somehow it does?
        let sender = SplitSend::from(
            Just::default()
                | Then::from({
                    let invocations = invocations.clone();
                    move |_| -> Result<(i32,)> {
                        let mut guarded_invocations = invocations.lock().unwrap();
                        assert_eq!(0, *guarded_invocations);
                        *guarded_invocations += 1;
                        Err(new_error(ErrorForTesting::from("bla bla chocoladevla")))
                    }
                }),
        );
        assert_eq!(0, *invocations.lock().unwrap(), "Split mustn't run yet.");

        let s1 = sender.clone() | Then::from(|(invocations,)| (invocations, invocations));
        let s2 = sender | Then::from(|(invocations,)| (invocations, invocations));

        s1.sync_wait_send().expect_err("an error should be yielded");
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split ran its sender-chain once."
        );

        s2.sync_wait_send()
            .expect_err("an error should be yielded the second time");
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split didn't run its sender-chain a second time."
        );
    }

    #[test]
    fn it_propagates_done_send() {
        let invocations = Arc::new(Mutex::new(0_i32)); // XXX shouldn't require Arc here, but somehow it does?
        let sender = SplitSend::from(
            Just::default()
                | LetValue::from({
                    let invocations = invocations.clone();
                    move |sch: ImmediateScheduler, _: refs::ScopedRefMut<(), refs::SendState>| {
                        let mut guarded_invocations = invocations.lock().unwrap();
                        assert_eq!(0, *guarded_invocations);
                        *guarded_invocations += 1;
                        sch.schedule_done::<(i32,)>()
                    }
                }),
        );
        assert_eq!(0, *invocations.lock().unwrap(), "Split mustn't run yet.");

        let s1 = sender.clone() | Then::from(|(invocations,)| (invocations, invocations));
        let s2 = sender | Then::from(|(invocations,)| (invocations, invocations));

        assert_eq!(
            None,
            s1.sync_wait_send().expect("no error"),
            "expect done signal"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split ran its sender-chain once."
        );

        assert_eq!(
            None,
            s2.sync_wait_send().expect("no error"),
            "expect done signal the second time"
        );
        assert_eq!(
            1,
            *invocations.lock().unwrap(),
            "Split didn't run its sender-chain a second time."
        );
    }
}
