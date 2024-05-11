pub trait Receiver {
    // Common receiver logic.
    fn set_done(self);
}

pub trait ReceiverOf<Tuple>: Receiver {
    fn set_value(self, values: Tuple);
}

pub trait ReceiverOfError<Error>: Receiver {
    fn set_error(self, error: Error);
}

pub trait OperationState {
    fn start(self);
}

pub trait Sender {}

pub trait TypedSender<Value, Error> {
    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Value> + ReceiverOfError<Error>;
}
