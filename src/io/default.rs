use crate::scheduler::Scheduler;

/// Schedulers implementing this trait, will receive a default implementation for [Write](crate::io::write::Write).
///
/// If you don't derive your [Scheduler] from [EnableDefaultIO],
/// you'll have to provide your own implementation of [Write](crate::io::write::Write).
pub trait EnableDefaultIO: Scheduler {}
