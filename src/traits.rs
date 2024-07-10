use crate::errors::{Error, Tuple};
use crate::scheduler::Scheduler;
use crate::scope::Scope;

/// Common receiver logic.
/// All receivers can accept the done signal, and the error signal.
pub trait Receiver {
    /// Accept the `done` signal.
    fn set_done(self);
    /// Accept an `error` signal.
    fn set_error(self, error: Error);
}

/// Declare that this is a receiver that can accept a specific `Values` type.
///
/// The value will be received, while running on the [Scheduler].
pub trait ReceiverOf<Sch: Scheduler, Values: Tuple>: Receiver {
    /// Accept a `value` signal.
    fn set_value(self, scheduler: Sch, values: Values);
}

/// An operation state is a [TypedSender] with matching [ReceiverOf].
/// It's ready to run, just waiting to be started.
pub trait OperationState<'a> {
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
/// Can be connected with a receiver, which is handled by the [TypedSenderConnect] trait.
pub trait TypedSender<'a> {
    /// The type of the value signal.
    type Value: Tuple;
    /// The scheduler for this sender.
    type Scheduler: Scheduler;
}

/// Trait for implementing `connect` functionality.
///
/// Senders are allowed to be arbitrarily restrictive about what type of receiver they'll accept.
/// (This is how we can make cross-thread schedulers require a receiver to implement [Send],
/// without requiring this trait on receivers for schedulers that don't require it.)
pub trait TypedSenderConnect<'scope, 'a, ScopeImpl, ReceiverType>: TypedSender<'a>
where
    'a: 'scope,
    ReceiverType: 'scope + ReceiverOf<Self::Scheduler, Self::Value>,
    ScopeImpl: Scope<'scope, 'a>,
{
    /// Attach a receiver.
    /// Will produce an operation state, that, once started, will invoke the receiver exactly once.
    fn connect(self, scope: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope>;
}

/// [Sender] can extend [TypedSender].
/// In order to do that, a function is invoked on that sender, with the typed sender as an argument.
/// BindSender models the binding of the sender with a typed sender.
///
/// The NestedSender argument should be a [TypedSender].
pub trait BindSender<NestedSender>: Sender {
    /// Result type of the bind operation.
    /// Should be some [TypedSender].
    type Output;

    /// Attach to a typed sender, creating a new typed sender.
    fn bind(self, nested: NestedSender) -> Self::Output;
}
