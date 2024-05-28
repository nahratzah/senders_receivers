pub mod same_thread_channel;

use crate::errors::{Error, IsTuple};
use crate::scheduler::Scheduler;
use crate::traits::{OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect};
use std::sync::mpsc;

enum SyncWaitOutcome<Tuple: IsTuple> {
    Value(Tuple),
    Error(Error),
    Done,
}

pub struct NoSendReceiver<Tuple: IsTuple> {
    tx: same_thread_channel::Sender<SyncWaitOutcome<Tuple>>,
}

impl<Tuple: IsTuple> Receiver for NoSendReceiver<Tuple> {
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

impl<Sch: Scheduler, Tuple: IsTuple> ReceiverOf<Sch, Tuple> for NoSendReceiver<Tuple> {
    fn set_value(self, _: Sch, values: Tuple) {
        self.tx
            .send(SyncWaitOutcome::Value(values))
            .expect("send must succeed");
    }
}

pub struct SendReceiver<Tuple: IsTuple + Send + 'static> {
    tx: mpsc::SyncSender<SyncWaitOutcome<Tuple>>,
}

impl<Tuple: IsTuple + Send + 'static> Receiver for SendReceiver<Tuple> {
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

impl<Sch: Scheduler, Tuple: IsTuple + Send + 'static> ReceiverOf<Sch, Tuple>
    for SendReceiver<Tuple>
{
    fn set_value(self, _: Sch, values: Tuple) {
        self.tx
            .send(SyncWaitOutcome::Value(values))
            .expect("send must succeed");
    }
}

/// Execute a [TypedSender], and yield the outcome of the operation.
///
/// The return type is a `Result<Option<..>, Error>`.  
/// If the typed sender yields an error, it'll be in the error position of the Result.  
/// Otherwise, if the operation is canceled (`done` signal), an empty Option will be returned.  
/// Otherwise the operation succeeds, and an `Ok(Some(..))` is returned.
///
/// This function requires that the send operation completes on the same thread.
/// (And without using a queue-scheduler.)
/// It'll accept a value that doesn't implement [Send] (for example [Rc](std::rc::Rc) values).
/// If you do need to cross threads, you'll need to use [sync_wait_send] instead.
///
/// Example:
/// ```
/// use senders_receivers::{Just, sync_wait};
/// use std::rc::Rc;
///
/// let sender = Just::from((Rc::new(String::from("bla")),));
/// match sync_wait(sender) {
///     Err(e) => println!("error signal: {:?}", e),
///     Ok(None) => println!("done signal"),
///     Ok(Some(tuple)) => println!("value signal: {:?}", tuple), // tuple: Rc<String> holding "bla"
/// };
/// ```
pub fn sync_wait<SenderImpl>(sender: SenderImpl) -> Result<Option<SenderImpl::Value>, Error>
where
    SenderImpl:
        TypedSender + TypedSenderConnect<NoSendReceiver<<SenderImpl as TypedSender>::Value>>,
{
    type SenderType<Value> = same_thread_channel::Sender<SyncWaitOutcome<Value>>;
    type ReceiverType<Value> = same_thread_channel::Receiver<SyncWaitOutcome<Value>>;

    let (tx, rx): (
        SenderType<SenderImpl::Value>,
        ReceiverType<SenderImpl::Value>,
    ) = same_thread_channel::channel(1);
    let receiver = NoSendReceiver { tx };
    sender.connect(receiver).start();
    match rx.recv().expect("a single value must be delivered") {
        SyncWaitOutcome::Value(tuple) => Ok(Some(tuple)),
        SyncWaitOutcome::Error(error) => Err(error),
        SyncWaitOutcome::Done => Ok(None),
    }
}

/// Execute a [TypedSender], and yield the outcome of the operation.
///
/// The return type is a `Result<Option<..>, Error>`.  
/// If the typed sender yields an error, it'll be in the error position of the Result.  
/// Otherwise, if the operation is canceled (`done` signal), an empty Option will be returned.  
/// Otherwise the operation succeeds, and an `Ok(Some(..))` is returned.
///
/// This function allows the [TypedSender] to cross thread-boundaries,
/// and requires the returned value to implement [Send].
/// If you don't need to cross thread-boundaries,
/// or can't (due to value not implementing [Send])
/// using [sync_wait] might be a better choice.
///
/// Example:
/// ```
/// use senders_receivers::{Just, sync_wait_send};
///
/// let sender = Just::from((String::from("bla"),));
/// match sync_wait_send(sender) {
///     Err(e) => println!("error signal: {:?}", e),
///     Ok(None) => println!("done signal"),
///     Ok(Some(tuple)) => println!("value signal: {:?}", tuple), // tuple: String holding "bla"
/// };
/// ```
pub fn sync_wait_send<SenderImpl>(sender: SenderImpl) -> Result<Option<SenderImpl::Value>, Error>
where
    SenderImpl: TypedSender + TypedSenderConnect<SendReceiver<<SenderImpl as TypedSender>::Value>>,
    <SenderImpl as TypedSender>::Value: Send + 'static,
{
    type SenderType<Value> = mpsc::SyncSender<SyncWaitOutcome<Value>>;
    type ReceiverType<Value> = mpsc::Receiver<SyncWaitOutcome<Value>>;

    let (tx, rx): (
        SenderType<SenderImpl::Value>,
        ReceiverType<SenderImpl::Value>,
    ) = mpsc::sync_channel(1);
    let receiver = SendReceiver { tx };
    sender.connect(receiver).start();
    match rx.recv().expect("a single value must be delivered") {
        SyncWaitOutcome::Value(tuple) => Ok(Some(tuple)),
        SyncWaitOutcome::Error(error) => Err(error),
        SyncWaitOutcome::Done => Ok(None),
    }
}
