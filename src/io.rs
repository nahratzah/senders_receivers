//! The `io` mod holds IO utilities.
//!
//! It decorates read/write/etc types, such that they play nice with [senders/receivers](TypedSender).
//! For example, it'll add a `fd.write(Scheduler, buffer)` (yielding a [TypedSender] to the [File](std::fs::File) type.

mod default;
mod write;

pub use default::EnableDefaultIO;
pub use write::Write;
