use crate::errors::{Error, IsTuple};
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use std::marker::PhantomData;
use std::ops::BitOr;

/// A typed-sender that holds a tuple of values.
///
/// Typically, this is the starting point of a sender chain.
///
/// ```
/// use senders_receivers::{Just, sync_wait};
///
/// let sender = Just::from((1, 2, 3));  // Create a typed sender returning a tuple of three values.
/// assert_eq!(
///     (1, 2, 3),
///     sync_wait(sender).unwrap().unwrap());
/// ```
///
/// If you want to start values on a specific scheduler, use [Just::with_scheduler]:
/// ```
/// use senders_receivers::{Just, WithScheduler, sync_wait_send};
/// use threadpool::ThreadPool;
///
/// let pool = ThreadPool::with_name("example".into(), 1);
/// let sender = Just::with_scheduler(pool, (1, 2, 3));
/// assert_eq!(
///     (1, 2, 3),
///     sync_wait_send(sender).unwrap().unwrap());
/// ```
pub struct Just<Sch: Scheduler, Tuple: IsTuple> {
    sch: Sch,
    values: Tuple,
}

impl Default for Just<ImmediateScheduler, ()> {
    fn default() -> Self {
        Just::from(())
    }
}

impl<Tuple: IsTuple> From<Tuple> for Just<ImmediateScheduler, Tuple> {
    /// Create a new typed sender, that emits the `init` value.
    fn from(init: Tuple) -> Self {
        Just {
            sch: ImmediateScheduler::default(),
            values: init,
        }
    }
}

impl<Sch: Scheduler, Tuple: IsTuple> WithScheduler<Sch, Tuple> for Just<Sch, Tuple> {
    /// Create a new typed sender, that emits the `init` value.
    ///
    /// Instead of using this function, you could also use [Scheduler::schedule_value].
    fn with_scheduler(sch: Sch, init: Tuple) -> Self {
        Just { sch, values: init }
    }
}

impl<Sch: Scheduler, Tuple: IsTuple> TypedSender for Just<Sch, Tuple> {
    type Value = Tuple;
    type Scheduler = Sch::LocalScheduler;
}

impl<ReceiverType, Sch, Tuple> TypedSenderConnect<ReceiverType> for Just<Sch, Tuple>
where
    Sch: Scheduler,
    Tuple: IsTuple,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Tuple>,
    Sch::Sender: TypedSenderConnect<ReceiverWrapper<ReceiverType, Sch::LocalScheduler, Tuple>>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        let receiver = ReceiverWrapper {
            phantom: PhantomData,
            receiver,
            values: self.values,
        };
        self.sch.schedule().connect(receiver)
    }
}

impl<BindSenderImpl, Sch, Tuple> BitOr<BindSenderImpl> for Just<Sch, Tuple>
where
    Sch: Scheduler,
    Tuple: IsTuple,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<ReceiverImpl, Sch, Tuple>
where
    ReceiverImpl: ReceiverOf<Sch, Tuple>,
    Tuple: IsTuple,
    Sch: Scheduler,
{
    phantom: PhantomData<fn(Sch)>,
    receiver: ReceiverImpl,
    values: Tuple,
}

impl<ReceiverImpl, Sch, Tuple> Receiver for ReceiverWrapper<ReceiverImpl, Sch, Tuple>
where
    ReceiverImpl: ReceiverOf<Sch, Tuple>,
    Tuple: IsTuple,
    Sch: Scheduler,
{
    fn set_done(self) {
        self.receiver.set_done();
    }

    fn set_error(self, error: Error) {
        self.receiver.set_error(error);
    }
}

impl<ReceiverImpl, Sch, Tuple> ReceiverOf<Sch, ()> for ReceiverWrapper<ReceiverImpl, Sch, Tuple>
where
    ReceiverImpl: ReceiverOf<Sch, Tuple>,
    Tuple: IsTuple,
    Sch: Scheduler,
{
    fn set_value(self, sch: Sch, _: ()) {
        self.receiver.set_value(sch, self.values);
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
            sync_wait(Just::from((4, 5, 6))).expect("just() should not fail")
        )
    }
}
