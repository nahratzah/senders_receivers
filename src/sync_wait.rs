use crate::errors::{Error, IsTuple};
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

impl<Tuple: IsTuple> ReceiverOf<Tuple> for SyncWaitAcceptorReceiver<'_, Tuple> {
    fn set_value(self, values: Tuple) {
        *self.acceptor = SyncWaitAcceptor::Value(values);
    }
}

pub fn sync_wait<SenderImpl>(
    sender: SenderImpl,
) -> Result<Option<<SenderImpl as TypedSender>::Value>, Error>
where
    SenderImpl: TypedSender,
{
    let mut acceptor: SyncWaitAcceptor<<SenderImpl as TypedSender>::Value> =
        SyncWaitAcceptor::Uninitialized;

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
