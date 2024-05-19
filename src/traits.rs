use crate::errors::{Error, IsTuple};
use crate::scheduler::Scheduler;

/// Common receiver logic.
/// All receivers can accept the done signal, and the error signal.
pub trait Receiver {
    // Accept the `done` signal.
    fn set_done(self);
    // Accept an `error` signal.
    fn set_error(self, error: Error);
}

/// Declare that this is a receiver that can accept a specific Value type.
///
/// The value will be received, while running on the [Scheduler].
pub trait ReceiverOf<Sch: Scheduler, Tuple: IsTuple>: Receiver {
    // Accept a `value` signal.
    fn set_value(self, scheduler: Sch, values: Tuple);
}

/// An operation state is a [TypedSender] with matching [ReceiverOf].
/// It's ready to run, just waiting to be started.
pub trait OperationState {
    /// Start the operation.
    fn start(self);
}

/// A sender is a type that describes a step in an operation.
///
/// A [TypedSender] can be extended with senders, creating a new [TypedSender].
/// This binding is implemented via the [BindSender] trait.
pub trait Sender {}

/// A typed sender is a sender, which describes an entire operation.
///
/// It can be extended with additional steps, by binding a [Sender] to it.
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

/// [Sender] can extend [TypedSender].
/// In order to do that, a function is invoked on that sender, with the typed sender as an argument.
/// BindSender models the binding of the sender with a typed sender.
pub trait BindSender<NestedSender: TypedSender>: Sender {
    // Result type of the bind operation.
    type Output: TypedSender;

    /// Attach to a typed sender, creating a new typed sender.
    fn bind(self, nested: NestedSender) -> Self::Output;
}
