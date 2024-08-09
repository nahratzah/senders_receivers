use crate::errors::Error;
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::stop_token::StopToken;
use crate::traits::{BindSender, Receiver, ReceiverOf, TypedSender, TypedSenderConnect};
use crate::tuple::Tuple;
use std::marker::PhantomData;
use std::ops::BitOr;

/// A typed-sender that holds a tuple of values.
///
/// Typically, this is the starting point of a sender chain.
///
/// ```
/// use senders_receivers::{Just, SyncWait};
///
/// let sender = Just::from((1, 2, 3));  // Create a typed sender returning a tuple of three values.
/// assert_eq!(
///     (1, 2, 3),
///     sender.sync_wait().unwrap().unwrap());
/// ```
///
/// If you want to start values on a specific scheduler, use [Just::with_scheduler]:
/// ```
/// use senders_receivers::{Just, WithScheduler, SyncWaitSend};
/// use threadpool::ThreadPool;
///
/// let pool = ThreadPool::with_name("example".into(), 1);
/// let sender = Just::with_scheduler(pool, (1, 2, 3));
/// assert_eq!(
///     (1, 2, 3),
///     sender.sync_wait_send().unwrap().unwrap());
/// ```
pub struct Just<'a, Sch: Scheduler, Tpl: 'a + Tuple> {
    phantom: PhantomData<fn() -> &'a Tpl>,
    sch: Sch,
    values: Tpl,
}

impl Default for Just<'_, ImmediateScheduler, ()> {
    fn default() -> Self {
        Just::from(())
    }
}

impl<'a, Tpl: 'a + Tuple> From<Tpl> for Just<'a, ImmediateScheduler, Tpl> {
    /// Create a new typed sender, that emits the `init` value.
    fn from(init: Tpl) -> Self {
        Just {
            phantom: PhantomData,
            sch: ImmediateScheduler,
            values: init,
        }
    }
}

impl<'a, Sch: Scheduler, Tpl: 'a + Tuple> WithScheduler<Sch, Tpl> for Just<'a, Sch, Tpl> {
    /// Create a new typed sender, that emits the `init` value.
    ///
    /// Instead of using this function, you could also use [Scheduler::schedule_value].
    fn with_scheduler(sch: Sch, init: Tpl) -> Self {
        Just {
            sch,
            values: init,
            phantom: PhantomData,
        }
    }
}

impl<'a, Sch: Scheduler, Tpl: 'a + Tuple> TypedSender for Just<'a, Sch, Tpl> {
    type Value = Tpl;
    type Scheduler = Sch::LocalScheduler;
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType, Sch, Tpl>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType> for Just<'a, Sch, Tpl>
where
    Sch: Scheduler,
    Tpl: 'a + Tuple,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Tpl>,
    Sch::Sender: TypedSenderConnect<
        'a,
        ScopeImpl,
        StopTokenImpl,
        ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Tpl>,
    >,
    StopTokenImpl: StopToken,
{
    type Output<'scope> = <<Sch as Scheduler>::Sender
        as
	TypedSenderConnect<
            'a,
            ScopeImpl,
            StopTokenImpl,
            ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Tpl>,
        >
    >::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        stop_token: StopTokenImpl,
        receiver: ReceiverType,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope,
    {
        let receiver = ReceiverWrapper {
            phantom: PhantomData,
            receiver,
            values: self.values,
        };
        self.sch.schedule().connect(scope, stop_token, receiver)
    }
}

impl<'a, BindSenderImpl, Sch, Tpl> BitOr<BindSenderImpl> for Just<'a, Sch, Tpl>
where
    Sch: Scheduler,
    Tpl: 'a + Tuple,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ReceiverWrapper<'a, ReceiverImpl, Sch, Tpl>
where
    ReceiverImpl: ReceiverOf<Sch, Tpl>,
    Tpl: 'a + Tuple,
    Sch: Scheduler<LocalScheduler = Sch>,
{
    phantom: PhantomData<fn(Sch) -> &'a Tpl>,
    receiver: ReceiverImpl,
    values: Tpl,
}

impl<'a, ReceiverImpl, Sch, Tpl> Receiver for ReceiverWrapper<'a, ReceiverImpl, Sch, Tpl>
where
    ReceiverImpl: ReceiverOf<Sch, Tpl>,
    Tpl: 'a + Tuple,
    Sch: Scheduler<LocalScheduler = Sch>,
{
    fn set_done(self) {
        self.receiver.set_done();
    }

    fn set_error(self, error: Error) {
        self.receiver.set_error(error);
    }
}

impl<'a, ReceiverImpl, Sch, Tpl> ReceiverOf<Sch, ()> for ReceiverWrapper<'a, ReceiverImpl, Sch, Tpl>
where
    ReceiverImpl: ReceiverOf<Sch, Tpl>,
    Tpl: 'a + Tuple,
    Sch: Scheduler<LocalScheduler = Sch>,
{
    fn set_value(self, sch: Sch, _: ()) {
        self.receiver.set_value(sch, self.values);
    }
}

#[cfg(test)]
mod tests {
    use super::Just;
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((4, 5, 6)),
            Just::from((4, 5, 6))
                .sync_wait()
                .expect("just() should not fail")
        )
    }
}
