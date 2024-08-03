use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::tuple::Tuple;

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
pub trait ReceiverOf<Sch, Values>: Receiver
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
{
    /// Accept a `value` signal.
    fn set_value(self, scheduler: Sch, values: Values);
}

/// An operation state is a [TypedSender] with matching [ReceiverOf].
/// It's ready to run, just waiting to be started.
pub trait OperationState<'scope> {
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
pub trait TypedSender {
    /// The type of the value signal.
    type Value: Tuple;
    /// The scheduler for this sender.
    type Scheduler: Scheduler<LocalScheduler = Self::Scheduler>;
}

/// Trait for implementing `connect` functionality.
///
/// Senders are allowed to be arbitrarily restrictive about what type of receiver they'll accept.
/// (This is how we can make cross-thread schedulers require a receiver to implement [Send],
/// without requiring this trait on receivers for schedulers that don't require it.)
pub trait TypedSenderConnect<'a, ScopeImpl, ReceiverType>: TypedSender
where
    ReceiverType: ReceiverOf<Self::Scheduler, Self::Value>,
{
    /// The [OperationState] returned by [TypedSenderConnect::connect()].
    type Output<'scope>: 'scope + OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;

    /// Attach a receiver.
    /// Will produce an operation state, that, once started, will invoke the receiver exactly once.
    fn connect<'scope>(self, scope: &ScopeImpl, receiver: ReceiverType) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;
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
