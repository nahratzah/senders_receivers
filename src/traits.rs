use crate::errors::{Error, IsTuple};
use crate::scheduler::Scheduler;

/// Common receiver logic.
pub trait Receiver {
    // Accept the `done` signal.
    fn set_done(self);
    // Accept an `error` signal.
    fn set_error(self, error: Error);
}

/// Declare that this is a receiver that can accept a specific Value type.
pub trait ReceiverOf<Sch: Scheduler, Tuple: IsTuple>: Receiver {
    // Accept an `value` signal.
    fn set_value(self, scheduler: Sch, values: Tuple);
}

/// An operation state is a typed-sender with matching receiver.
/// It's ready to run, just waiting to be started.
pub trait OperationState {
    /// Start the operation.
    fn start(self);
}

/// A sender is a type that can be part of a sender chain.
/// A typed sender can be extended with senders, to create an operation.
pub trait Sender {}

/// A typed sender is a sender, which describes an operation.
pub trait TypedSender {
    /// The type of the value signal.
    type Value: IsTuple;
    /// The scheduler for this sender.
    type Scheduler: Scheduler;

    /// Attach a receiver.
    /// Will produce an operation state, that, once started, will invoke the receiver exactly once.
    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Self::Scheduler, Self::Value>;
}

/// Senders can extend typed senders.
/// In order to do that, a function is invoked on that sender, with the typed sender as an argument.
/// BindSender models the binding of the sender with a typed sender.
pub trait BindSender<NestedSender: TypedSender>: Sender {
    // Result type of the bind operation.
    type Output: TypedSender;

    /// Attach to a typed sender, creating a new typed sender.
    fn bind(self, nested: NestedSender) -> Self::Output;
}
