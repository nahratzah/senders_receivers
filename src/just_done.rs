use crate::errors::IsTuple;
use crate::scheduler::{ImmediateScheduler, Scheduler};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use std::marker::PhantomData;
use std::ops::BitOr;

/// A [TypedSender] that always generates a done-signal.
///
/// Example:
/// ```
/// use senders_receivers::{ImmediateScheduler, JustDone, sync_wait};
///
/// fn example() {
///     // The `::<(i32, i32)>` turbo-fish is to declare the value-type of the created sender.
///     let sender = JustDone::<ImmediateScheduler, (i32, i32)>::new();
///     match sync_wait(sender) {
///         Ok(Some(_)) => panic!("there won't be a value"),
///         Ok(None) => println!("completed with done signal"),  // This is returned.
///         Err(e) => panic!("error: {:?}", e),
///     };
/// }
/// ```
pub struct JustDone<Sch: Scheduler, Tuple: IsTuple> {
    phantom: PhantomData<fn(Sch) -> Tuple>,
}

impl<Sch: Scheduler, Tuple: IsTuple> JustDone<Sch, Tuple> {
    /// Create a new typed sender that'll yield an error.
    ///
    /// Since you usually need to use a turbo-fish to use this function,
    /// you might prefer using [Scheduler::schedule_done] instead.
    pub fn new() -> JustDone<Sch, Tuple> {
        JustDone {
            phantom: PhantomData,
        }
    }
}

impl<Tuple: IsTuple> Default for JustDone<ImmediateScheduler, Tuple> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Sch: Scheduler, Tuple: IsTuple> TypedSender for JustDone<Sch, Tuple> {
    type Value = Tuple;
    type Scheduler = Sch::LocalScheduler;
}

impl<Sch, ReceiverType, Tuple> TypedSenderConnect<ReceiverType> for JustDone<Sch, Tuple>
where
    Sch: Scheduler,
    Tuple: IsTuple,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Tuple>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        JustDoneOperationState { receiver }
    }
}

pub struct JustDoneOperationState<ReceiverImpl: Receiver> {
    receiver: ReceiverImpl,
}

impl<ReceiverImpl: Receiver> OperationState for JustDoneOperationState<ReceiverImpl> {
    fn start(self) {
        self.receiver.set_done()
    }
}

impl<Sch: Scheduler, Tuple: IsTuple, BindSenderImpl> BitOr<BindSenderImpl> for JustDone<Sch, Tuple>
where
    BindSenderImpl: BindSender<JustDone<Sch, Tuple>>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::JustDone;
    use crate::scheduler::ImmediateScheduler;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(
            None,
            sync_wait(JustDone::<ImmediateScheduler, ()>::default()).unwrap()
        )
    }
}
