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

pub trait TypedSender {
    type Value;
    type Error;

    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Self::Value> + ReceiverOfError<Self::Error>;
}

pub trait BindSender<NestedSender: TypedSender>: Sender {
    type Output: TypedSender;

    fn bind(self, nested: NestedSender) -> Self::Output;
}
