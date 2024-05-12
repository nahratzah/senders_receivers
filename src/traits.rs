use crate::errors::{Error, IsTuple};

// Common receiver logic.
pub trait Receiver {
    fn set_done(self);
    fn set_error(self, error: Error);
}

pub trait ReceiverOf<Tuple: IsTuple>: Receiver {
    fn set_value(self, values: Tuple);
}

pub trait OperationState {
    fn start(self);
}

pub trait Sender {}

pub trait TypedSender {
    type Value: IsTuple;

    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Self::Value>;
}

pub trait BindSender<NestedSender: TypedSender>: Sender {
    type Output: TypedSender;

    fn bind(self, nested: NestedSender) -> Self::Output;
}
