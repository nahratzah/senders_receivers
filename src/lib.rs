#![crate_name = "senders_receivers"]

//! This is an implementation (for certain value of implementation) of C++ senders/receivers.
//!
//! ## Comparison With C++
//! An attempt is made to remain close the the usability of the C++ style,
//! but sacrifices had to be made.
//!
//! - The `Error` type is dynamic.  
//!    For a non-dynamic error type, we would require a variant-type with variadic type arguments.
//! - The `Value` type is a tuple.  
//!   For a non-tuple type, we would require a variant-type with variadic type arguments.
//! - The `Value` is not a variant.  
//!   For a variant-type, we would require a variant-type with variadic type arguments.
//! - Scheduler is omitted.  
//!   I'm planning to add this.
//! - Scheduler-based specializations are omitted.  
//!   I might be able to, if I change the way the code is structured?
//! - Connect can no longer fail.  
//!   In C++, the `connect` call can fail, and this will result in an error being propagated.
//!   This meant that when the `connect` call inside a `let_value` step fails,
//!   the error would have to propage via the receiver.
//!   The same receiver would also be passed to the connect call, using move semantics.  
//!   In rust, this will cause the borrow checker to flag this as bad, and I kinda like the borrow checker.
//!   Instead I decided: if the `connect` fails, it'll have to create an operation state that'll propagate an error.
//!
//! Some sacrifices stem from me disagreeing with the C++ design.
//! I liked the promise from the design, that scheduler changes are explicit only.
//! But in practice, it was very hard to use, and my code, once async, always needed to grab the receiver-scheduler
//! (because it was too common for the sender not to have a scheduler).
//!
//! 1. The `done` and `error` channels no longer have an associated scheduler.
//! 2. The `value` channel now always has an associated scheduler.
//! 3. The receiver no longer has a scheduelr.
//!
//! The reason that `error` channels no longer have an associated scheduler,
//! is because scheduler-transfers can fail, and this would break the invariant of an error-scheduler.
//!
//! Dropping the scheduler from `done` and `error` signals, means that scheduler switches will only happen on the happy path
//! (and on recovery paths).

mod errors;
pub mod functor;
mod just;
mod just_done;
mod just_error;
mod let_value;
mod scheduler;
mod start_detached;
mod sync_wait;
mod then;
mod traits;
mod transfer;

pub use errors::{new_error, Error, IsTuple};
pub use just::Just;
pub use just_done::JustDone;
pub use just_error::JustError;
pub use let_value::LetValue;
pub use scheduler::{ImmediateScheduler, Scheduler};
pub use start_detached::start_detached;
pub use sync_wait::{sync_wait, sync_wait_send};
pub use then::Then;
pub use traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
pub use transfer::Transfer;
