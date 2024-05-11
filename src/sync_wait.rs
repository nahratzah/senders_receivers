use crate::traits::{OperationState, Receiver, ReceiverOf, ReceiverOfError, TypedSender};

enum SyncWaitAcceptor<Tuple, Error> {
    Uninitialized,
    Value(Tuple),
    Error(Error),
    Done,
}

struct SyncWaitAcceptorReceiver<'a, Tuple, Error> {
    acceptor: &'a mut SyncWaitAcceptor<Tuple, Error>,
}

impl<Tuple, Error> Receiver for SyncWaitAcceptorReceiver<'_, Tuple, Error> {
    fn set_done(self) {
        *self.acceptor = SyncWaitAcceptor::Done;
    }
}
impl<Tuple, Error> ReceiverOf<Tuple> for SyncWaitAcceptorReceiver<'_, Tuple, Error> {
    fn set_value(self, values: Tuple) {
        *self.acceptor = SyncWaitAcceptor::Value(values);
    }
}
impl<Tuple, Error> ReceiverOfError<Error> for SyncWaitAcceptorReceiver<'_, Tuple, Error> {
    fn set_error(self, error: Error) {
        *self.acceptor = SyncWaitAcceptor::Error(error);
    }
}

pub fn sync_wait<SenderImpl>(
    sender: SenderImpl,
) -> Result<Option<<SenderImpl as TypedSender>::Value>, <SenderImpl as TypedSender>::Error>
where
    SenderImpl: TypedSender,
{
    let mut acceptor: SyncWaitAcceptor<
        <SenderImpl as TypedSender>::Value,
        <SenderImpl as TypedSender>::Error,
    > = SyncWaitAcceptor::Uninitialized;

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
