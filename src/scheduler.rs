use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender};
use core::ops::BitOr;

/// Schedulers are things that can do work.
/// All sender chains run on a scheduler.
///
/// Most of the time, you'll use a scheduler in combination with `Transfer`.
/// But a scheduler can be used directly (which is what `Transfer` does under the hood).
/// Unfortunately, I can't make the | operator work for scheduler, so we must use the `bind` function instead.
/// ```
/// use senders_receivers::{BindSender, Scheduler, Then, sync_wait};
///
/// fn compute_expensive_thing<Sch, LocalSch>(myScheduler: Sch)
/// where
///     Sch: Scheduler<LocalScheduler = LocalSch>,
///     LocalSch: Scheduler<LocalScheduler = LocalSch>,
/// {
///     let sender = Then::new_fn(|()| {
///                      let mut s = 0_i32;
///                      for n in 1..101 {
///                          s += n;
///                      }
///                      (s,)
///                  }).bind(myScheduler.schedule());
///     assert_eq!(
///         5050,
///         sync_wait(sender).unwrap().unwrap().0);
/// }
/// ```
pub trait Scheduler {
    /// Mark if the scheduler may block the caller, when started.
    /// If this is `false`, you are guaranteed that the sender will complete independently from the operation-state start method.
    /// But if it returns `true`, the scheduler will complete before returning from the operation-state start method.
    const EXECUTION_BLOCKS_CALLER: bool;
    /// The scheduler that's passed on along the value signal.
    /// Most of the time, this would be the same as this.
    ///
    /// But if for example an embarrisingly-parallel scheduler is used,
    /// the LocalScheduler would represent the scheduler bound to the thread that was selected.
    type LocalScheduler: Scheduler<Sender = Self::Sender>;

    /// The typed-sender returned by the scheduler.
    type Sender: TypedSender<Scheduler = Self::LocalScheduler, Value = ()>;

    /// Create a sender that'll run on this scheduler.
    fn schedule(self) -> Self::Sender;
}

pub struct ImmediateScheduler {}

/// This scheduler is a basic scheduler, that just runs everything immediately.
impl Scheduler for ImmediateScheduler {
    const EXECUTION_BLOCKS_CALLER: bool = true;
    type LocalScheduler = ImmediateScheduler;
    type Sender = ImmediateSender;

    fn schedule(self) -> Self::Sender {
        ImmediateSender {}
    }
}

pub struct ImmediateSender {}

impl TypedSender for ImmediateSender {
    type Scheduler = ImmediateScheduler;
    type Value = ();

    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Self::Scheduler, Self::Value>,
    {
        ImmediateOperationState { receiver }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for ImmediateSender
where
    BindSenderImpl: BindSender<ImmediateSender>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ImmediateOperationState<ReceiverType>
where
    ReceiverType: ReceiverOf<ImmediateScheduler, ()>,
{
    receiver: ReceiverType,
}

impl<ReceiverType> OperationState for ImmediateOperationState<ReceiverType>
where
    ReceiverType: ReceiverOf<ImmediateScheduler, ()>,
{
    fn start(self) {
        self.receiver.set_value(ImmediateScheduler {}, ());
    }
}
