use crate::errors::{Error, IsTuple};
use crate::scheduler::Scheduler;
use crate::traits::{OperationState, Receiver, ReceiverOf, TypedSender};

enum SyncWaitAcceptor<Tuple: IsTuple> {
    Uninitialized,
    Value(Tuple),
    Error(Error),
    Done,
}

struct SyncWaitAcceptorReceiver<'a, Tuple: IsTuple> {
    acceptor: &'a mut SyncWaitAcceptor<Tuple>,
}

impl<Tuple: IsTuple> Receiver for SyncWaitAcceptorReceiver<'_, Tuple> {
    fn set_done(self) {
        *self.acceptor = SyncWaitAcceptor::Done;
    }

    fn set_error(self, error: Error) {
        *self.acceptor = SyncWaitAcceptor::Error(error);
    }
}

impl<Sch: Scheduler, Tuple: IsTuple> ReceiverOf<Sch, Tuple>
    for SyncWaitAcceptorReceiver<'_, Tuple>
{
    fn set_value(self, _: Sch, values: Tuple) {
        *self.acceptor = SyncWaitAcceptor::Value(values);
    }
}

/// Execute a [TypedSender], and yield the outcome of the operation.
///
/// The return type is a `Result<Option<..>, Error>`.  
/// If the typed sender yields an error, it'll be in the error position of the Result.  
/// Otherwise, if the operation is canceled (`done` signal), an empty Option will be returned.  
/// Otherwise the operation succeeds, and an `Ok(Some(..))` is returned.
///
/// Example:
/// ```
/// use senders_receivers::{Just, sync_wait};
///
/// let sender = Just::new(("bla",));
/// match sync_wait(sender) {
///     Err(e) => println!("error signal: {:?}", e),
///     Ok(None) => println!("done signal"),
///     Ok(Some(tuple)) => println!("value signal: {:?}", tuple), // tuple: &str "bla"
/// };
/// ```
pub fn sync_wait<SenderImpl>(sender: SenderImpl) -> Result<Option<SenderImpl::Value>, Error>
where
    SenderImpl: TypedSender,
{
    let mut acceptor: SyncWaitAcceptor<SenderImpl::Value> = SyncWaitAcceptor::Uninitialized;

    let acceptor_receiver = SyncWaitAcceptorReceiver {
        acceptor: &mut acceptor,
    };
    let op_state = sender.connect(acceptor_receiver);
    op_state.start();

    match acceptor {
        SyncWaitAcceptor::Uninitialized => panic!("acceptor was not filled in"),
        SyncWaitAcceptor::Value(tuple) => Ok(Some(tuple)),
        SyncWaitAcceptor::Error(error) => Err(error),
        SyncWaitAcceptor::Done => Ok(None),
    }
}
