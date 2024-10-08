use crate::scheduler::{ImmediateScheduler, Scheduler};
use crate::stop_token::StopToken;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use crate::tuple::Tuple;
use std::marker::PhantomData;
use std::ops::BitOr;

/// A [TypedSender] that always generates a done-signal.
///
/// Example:
/// ```
/// use senders_receivers::{ImmediateScheduler, JustDone, SyncWait};
///
/// fn example() {
///     // The `::<(i32, i32)>` turbo-fish is to declare the value-type of the created sender.
///     let sender = JustDone::<ImmediateScheduler, (i32, i32)>::new();
///     match sender.sync_wait() {
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

impl<Sch: Scheduler, Tpl: Tuple> TypedSender for JustDone<Sch, Tpl> {
    type Value = Tpl;
    type Scheduler = Sch::LocalScheduler;
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType, Sch, Tpl>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType> for JustDone<Sch, Tpl>
where
    Sch: Scheduler,
    Tpl: Tuple,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Tpl>,
    StopTokenImpl: StopToken,
{
    type Output<'scope> = JustDoneOperationState<'scope, ReceiverType>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope ;

    fn connect<'scope>(
        self,
        _: &ScopeImpl,
        _: StopTokenImpl,
        receiver: ReceiverType,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope,
    {
        JustDoneOperationState {
            phantom: PhantomData,
            receiver,
        }
    }
}

pub struct JustDoneOperationState<'scope, ReceiverImpl>
where
    ReceiverImpl: Receiver + 'scope,
{
    phantom: PhantomData<&'scope ()>,
    receiver: ReceiverImpl,
}

impl<'scope, ReceiverImpl> OperationState<'scope> for JustDoneOperationState<'scope, ReceiverImpl>
where
    ReceiverImpl: Receiver + 'scope,
{
    fn start(self) {
        self.receiver.set_done();
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
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_works() {
        assert_eq!(
            None,
            JustDone::<ImmediateScheduler, ()>::default()
                .sync_wait()
                .unwrap()
        )
    }
}
