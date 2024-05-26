use crate::errors::IsTuple;
use crate::scheduler::ImmediateScheduler;
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use std::marker::PhantomData;
use std::ops::BitOr;

/// A [TypedSender] that always generates a done-signal.
///
/// Example:
/// ```
/// use senders_receivers::{JustDone, sync_wait};
///
/// fn example() {
///     // The `::<(i32, i32)>` turbo-fish is to declare the value-type of the created sender.
///     let sender = JustDone::<(i32, i32)>::new();
///     match sync_wait(sender) {
///         Ok(Some(_)) => panic!("there won't be a value"),
///         Ok(None) => println!("completed with done signal"),  // This is returned.
///         Err(e) => panic!("error: {:?}", e),
///     };
/// }
/// ```
pub struct JustDone<Tuple: IsTuple> {
    phantom: PhantomData<fn() -> Tuple>,
}

impl<Tuple: IsTuple> JustDone<Tuple> {
    /// Create a new typed sender that'll yield an error.
    pub fn new() -> JustDone<Tuple> {
        JustDone {
            phantom: PhantomData,
        }
    }
}

impl<Tuple: IsTuple> Default for JustDone<Tuple> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Tuple: IsTuple> TypedSender for JustDone<Tuple> {
    type Value = Tuple;
    type Scheduler = ImmediateScheduler;
}

impl<ReceiverType, Tuple> TypedSenderConnect<ReceiverType> for JustDone<Tuple>
where
    Tuple: IsTuple,
    ReceiverType: ReceiverOf<ImmediateScheduler, Tuple>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        JustDoneOperationState {
            phantom: PhantomData,
            receiver,
        }
    }
}

pub struct JustDoneOperationState<Tuple: IsTuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<ImmediateScheduler, Tuple>,
{
    phantom: PhantomData<fn() -> Tuple>,
    receiver: ReceiverImpl,
}

impl<Tuple: IsTuple, ReceiverImpl> OperationState for JustDoneOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<ImmediateScheduler, Tuple>,
{
    fn start(self) {
        self.receiver.set_done()
    }
}

impl<Tuple: IsTuple, BindSenderImpl> BitOr<BindSenderImpl> for JustDone<Tuple>
where
    BindSenderImpl: BindSender<JustDone<Tuple>>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::JustDone;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(None, sync_wait(JustDone::<()>::new()).unwrap())
    }
}
