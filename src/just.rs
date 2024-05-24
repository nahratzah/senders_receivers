use crate::errors::IsTuple;
use crate::scheduler::ImmediateScheduler;
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use core::ops::BitOr;

/// A typed-sender that holds a tuple of values.
///
/// Typically, this is the starting point of a sender chain.
///
/// ```
/// use senders_receivers::{Just, sync_wait};
///
/// let sender = Just::new((1, 2, 3));  // Create a typed sender returning a tuple of three values.
/// assert_eq!(
///     (1, 2, 3),
///     sync_wait(sender).unwrap().unwrap());
/// ```
pub struct Just<Tuple: IsTuple> {
    values: Tuple,
}

impl<Tuple: IsTuple> Just<Tuple> {
    /// Create a new typed sender, that emits the `init` value.
    pub fn new(init: Tuple) -> Just<Tuple> {
        Just::<Tuple> { values: init }
    }
}

impl<Tuple: IsTuple> TypedSender for Just<Tuple> {
    type Value = Tuple;
    type Scheduler = ImmediateScheduler;
}

impl<ReceiverType, Tuple> TypedSenderConnect<ReceiverType> for Just<Tuple>
where
    Tuple: IsTuple,
    ReceiverType: ReceiverOf<ImmediateScheduler, Tuple>,
{
    fn connect_two(self, receiver: ReceiverType) -> impl OperationState {
        JustOperationState {
            values: self.values,
            receiver,
        }
    }
}

pub struct JustOperationState<Tuple: IsTuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<ImmediateScheduler, Tuple>,
{
    values: Tuple,
    receiver: ReceiverImpl,
}

impl<Tuple: IsTuple, ReceiverImpl> OperationState for JustOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<ImmediateScheduler, Tuple>,
{
    fn start(self) {
        self.receiver.set_value(ImmediateScheduler {}, self.values)
    }
}

impl<Tuple: IsTuple, BindSenderImpl> BitOr<BindSenderImpl> for Just<Tuple>
where
    BindSenderImpl: BindSender<Just<Tuple>>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Just;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((4, 5, 6)),
            sync_wait(Just::new((4, 5, 6))).expect("just() should not fail")
        )
    }
}
