use crate::just::Just;
use crate::traits::TypedSender;

/// Schedulers are things that can do work.
/// All sender chains run on a scheduler.
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
    type LocalScheduler: Scheduler;

    /// Create a sender that'll run on this scheduler.
    fn schedule(self) -> impl TypedSender<Scheduler = Self::LocalScheduler, Value = ()>;
}

pub struct ImmediateScheduler {}

/// This scheduler is a basic scheduler, that just runs everything immediately.
impl Scheduler for ImmediateScheduler {
    const EXECUTION_BLOCKS_CALLER: bool = true;
    type LocalScheduler = ImmediateScheduler;

    fn schedule(self) -> impl TypedSender<Scheduler = Self::LocalScheduler, Value = ()> {
        return Just::new(());
    }
}
