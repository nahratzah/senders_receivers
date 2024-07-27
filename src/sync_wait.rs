use crate::errors::{Error, Result};
use crate::scheduler::Scheduler;
use crate::scope::scope_data::{ScopeDataPtr, ScopeDataSendPtr};
use crate::scope::ScopeImpl;
use crate::scope::{scope, scope_send};
use crate::sync::same_thread_channel;
use crate::traits::{OperationState, Receiver, ReceiverOf, TypedSenderConnect};
use crate::tuple::Tuple;
use std::sync::mpsc;

enum SyncWaitOutcome<Tpl: Tuple> {
    Value(Tpl),
    Error(Error),
    Done,
}

pub struct NoSendReceiver<Tpl: Tuple> {
    tx: same_thread_channel::Sender<SyncWaitOutcome<Tpl>>,
}

impl<Tpl: Tuple> NoSendReceiver<Tpl> {
    fn install_outcome(self, outcome: SyncWaitOutcome<Tpl>) {
        self.tx.send(outcome).expect("send must succeed");
    }
}

impl<Tpl: Tuple> Receiver for NoSendReceiver<Tpl> {
    fn set_done(self) {
        self.install_outcome(SyncWaitOutcome::Done);
    }

    fn set_error(self, error: Error) {
        self.install_outcome(SyncWaitOutcome::Error(error));
    }
}

impl<Sch: Scheduler, Tpl: Tuple> ReceiverOf<Sch, Tpl> for NoSendReceiver<Tpl> {
    fn set_value(self, _: Sch, values: Tpl) {
        self.install_outcome(SyncWaitOutcome::Value(values));
    }
}

pub struct SendReceiver<Tpl: Tuple + Send> {
    tx: mpsc::SyncSender<SyncWaitOutcome<Tpl>>,
}

impl<Tpl: Tuple + Send> Receiver for SendReceiver<Tpl> {
    fn set_done(self) {
        self.tx
            .send(SyncWaitOutcome::Done)
            .expect("send must succeed");
    }

    fn set_error(self, error: Error) {
        self.tx
            .send(SyncWaitOutcome::Error(error))
            .expect("send must succeed");
    }
}

impl<Sch: Scheduler, Tpl: Tuple + Send> ReceiverOf<Sch, Tpl> for SendReceiver<Tpl> {
    fn set_value(self, _: Sch, values: Tpl) {
        self.tx
            .send(SyncWaitOutcome::Value(values))
            .expect("send must succeed");
    }
}

/// Trait that implements the `sync_wait` method.
pub trait SyncWait<'a, Value>
where
    Value: Tuple,
{
    /// Execute a [TypedSender](crate::traits::TypedSender), and yield the outcome of the operation.
    ///
    /// The return type is a `Result<Option<..>, Error>`.  
    /// If the typed sender yields an error, it'll be in the error position of the Result.  
    /// Otherwise, if the operation is canceled (`done` signal), an empty Option will be returned.  
    /// Otherwise the operation succeeds, and an `Ok(Some(..))` is returned.
    ///
    /// This function requires that the send operation completes on the same thread.
    /// It'll accept a value that doesn't implement [Send] (for example [Rc](std::rc::Rc) values).
    /// If you do need to cross threads, you'll need to use [SyncWaitSend::sync_wait_send()] instead.
    ///
    /// Example:
    /// ```
    /// use senders_receivers::{Just, SyncWait};
    /// use std::rc::Rc;
    ///
    /// let sender = Just::from((Rc::new(String::from("bla")),));
    /// match sender.sync_wait() {
    ///     Err(e) => println!("error signal: {:?}", e),
    ///     Ok(None) => println!("done signal"),
    ///     Ok(Some(tuple)) => println!("value signal: {:?}", tuple), // tuple: Rc<String> holding "bla"
    /// };
    /// ```
    fn sync_wait(self) -> Result<Option<Value>>;
}

impl<'a, Value, T> SyncWait<'a, Value> for T
where
    Value: 'a + Tuple,
    T: TypedSenderConnect<'a, ScopeImpl<ScopeDataPtr>, NoSendReceiver<Value>, Value = Value>,
{
    fn sync_wait(self) -> Result<Option<Value>> {
        scope(move |scope: &ScopeImpl<ScopeDataPtr>| {
            let (tx, rx) = same_thread_channel::channel(1);
            let receiver = NoSendReceiver { tx };
            self.connect(scope, receiver).start();
            match rx.recv().expect("a single value must be delivered") {
                SyncWaitOutcome::Value(tuple) => Ok(Some(tuple)),
                SyncWaitOutcome::Error(error) => Err(error),
                SyncWaitOutcome::Done => Ok(None),
            }
        })
    }
}

/// Trait that implements the `sync_wait_send` method.
pub trait SyncWaitSend<'a, Value>
where
    Value: Tuple,
{
    /// Execute a [TypedSender](crate::traits::TypedSender), and yield the outcome of the operation.
    ///
    /// The return type is a `Result<Option<..>, Error>`.  
    /// If the typed sender yields an error, it'll be in the error position of the Result.  
    /// Otherwise, if the operation is canceled (`done` signal), an empty Option will be returned.  
    /// Otherwise the operation succeeds, and an `Ok(Some(..))` is returned.
    ///
    /// This function allows the sender-chain to switch threads.
    ///
    /// Example:
    /// ```
    /// use senders_receivers::{Just, SyncWait};
    /// use std::rc::Rc;
    ///
    /// let sender = Just::from((Rc::new(String::from("bla")),));
    /// match sender.sync_wait() {
    ///     Err(e) => println!("error signal: {:?}", e),
    ///     Ok(None) => println!("done signal"),
    ///     Ok(Some(tuple)) => println!("value signal: {:?}", tuple), // tuple: Rc<String> holding "bla"
    /// };
    /// ```
    fn sync_wait_send(self) -> Result<Option<Value>>;
}

impl<'a, Value, T> SyncWaitSend<'a, Value> for T
where
    Value: 'a + Tuple + Send,
    T: for<'scope> TypedSenderConnect<
        'a,
        ScopeImpl<ScopeDataSendPtr>,
        SendReceiver<Value>,
        Value = Value,
    >,
{
    fn sync_wait_send(self) -> Result<Option<Value>> {
        scope_send(move |scope: &ScopeImpl<ScopeDataSendPtr>| {
            let (tx, rx) = mpsc::sync_channel(1);
            let receiver = SendReceiver { tx };
            self.connect(scope, receiver).start();
            match rx.recv().expect("a single value must be delivered") {
                SyncWaitOutcome::Value(tuple) => Ok(Some(tuple)),
                SyncWaitOutcome::Error(error) => Err(error),
                SyncWaitOutcome::Done => Ok(None),
            }
        })
    }
}
