use crate::errors::Tuple;
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
pub struct JustDone<Sch: Scheduler, Tpl: Tuple> {
    phantom: PhantomData<fn(Sch) -> Tpl>,
}

impl<Sch: Scheduler, Tpl: Tuple> JustDone<Sch, Tpl> {
    /// Create a new typed sender that'll yield an error.
    ///
    /// Since you usually need to use a turbo-fish to use this function,
    /// you might prefer using [Scheduler::schedule_done] instead.
    pub fn new() -> JustDone<Sch, Tpl> {
        JustDone {
            phantom: PhantomData,
        }
    }
}

impl<Tpl: Tuple> Default for JustDone<ImmediateScheduler, Tpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Sch: Scheduler, Tpl: Tuple> TypedSender<'_> for JustDone<Sch, Tpl> {
    type Value = Tpl;
    type Scheduler = Sch::LocalScheduler;
}

impl<Sch, ReceiverType, Tpl> TypedSenderConnect<'_, ReceiverType> for JustDone<Sch, Tpl>
where
    Sch: Scheduler,
    Tpl: Tuple,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Tpl>,
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

impl<Sch: Scheduler, Tpl: Tuple, BindSenderImpl> BitOr<BindSenderImpl> for JustDone<Sch, Tpl>
where
    BindSenderImpl: BindSender<Self>,
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
