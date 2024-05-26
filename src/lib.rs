#![crate_name = "senders_receivers"]

//! This is an implementation (for certain value of implementation) of C++ senders/receivers.
//!
//! ## A Tiny Example
//! ```
//! use senders_receivers::*;
//!
//! let sender = Just::from((1, 2, 3, 4))
//!            | Then::from(|(a, b, c, d)| (a * b * c * d,));
//! println!("outcome: {}", sync_wait(sender).expect("no error").expect("no cancelation").0);
//! ```
//!
//! What this does:
//! - `Just::from`: declares an starting value for the sender chain.
//! - `Then::from`: declares a transformation on these values.
//! - `|` (the pipe symbol): used to bind them together.
//!
//! None of the steps are run, until `sync_wait` is invoked.
//!
//! ## Senders? Receivers?
//! The main concept of this code comes from having both senders and receivers.
//! A sender is a type that produces a value signal (or an error signal, or a done signal).
//! A receiver is a type that accepts this signal.
//!
//! Senders are composable: they can be chained together, creating a new sender.
//! This is done with the `|` (pipe) symbol.
//!
//! If you are using the library, you will not spot any receivers, unless you're implementing your own extensions.
//!
//! ## Signals
//! Each sender, produces either
//! a `value signal`,
//! an `error signal`,
//! or a `done signal`.
//! Exactly one of these will be produced.
//!
//! - A `value signal` indicates that the sender completed successfully, and produced a value.  
//! - An `error signal` indicates that the sender failed, and produced an [Error].  
//! - A `done signal` indicates that the sender canceled its work (producing neither a value, nor an error).
//!
//! ## Schedulers
//! Each operation will run on a [Scheduler].
//! (Sometimes more than one, for example if you [Transfer] to a different scheduler.)
//!
//! A scheduler encapsulates the concept of CPU-time.
//! Different schedulers will have different characteristics, related to when/where things run.
//!
//! Currently, the following schedulers are implemented:
//! - [ImmediateScheduler] runs every task immediately. This is more-or-less the default scheduler.
//! - [ThreadPool](threadpool::ThreadPool) runs tasks using a threadpool.
//!
//! ## Comparison With C++
//! An attempt is made to remain close the the usability of the C++ style,
//! but sacrifices had to be made.
//!
//! - The `Error` type is type-erased/dynamic.  
//!   For a non-dynamic error type, we would require a variant-type with variadic type arguments.
//! - The `Value` type is a tuple.  
//!   For a non-tuple type, we would require variadic type arguments.
//! - The `Value` is not a variant.  
//!   For a variant-type, we would require a variant-type with variadic type arguments.
//! - Scheduler-based specializations are omitted.  
//!   I think for this, we would require specializations.
//! - Connect can no longer fail.  
//!   In C++, the `connect` call can fail, and this will result in an error being propagated.
//!   This meant that when the `connect` call inside a `let_value` step fails,
//!   the error would have to propage via the receiver.
//!   The same receiver would also be passed to the connect call, using move semantics.  
//!   In rust, this will cause the borrow checker to flag this as bad, and I kinda like the borrow checker.
//!   Instead I decided: if the `connect` fails, it'll have to create an operation state that'll propagate an error.
//! - Operation-states don't nest themselves.  
//!   Mostly, the constraint stems from rust being very picky about move semantics (and I approve).
//!   And also, rust does not allow in-place construction (which is required for nested operation states; the C++ design requires in-place construction).
//!   Instead we wrap subsequent operations into a receiver, during the connect call.
//!   This has the small disadvantage we cannot do any preparations during the [OperationState::start] step.
//!
//! Some sacrifices stem from me disagreeing with the C++ design.
//! I liked the promise from the design, that scheduler changes are explicit only.
//! But in practice, it was very hard to use, and my code, once async, always needed to grab the receiver-scheduler
//! (because it was too common for the sender not to have a scheduler).
//!
//! 1. The `done` and `error` channels no longer have an associated scheduler.  
//!    There is no way to properly guarantee which scheduler an error is propagated upon.
//! 2. The `value` channel now always has an associated scheduler.  
//!    This allows for example a non-blocking write (ex: `aio_write`) to suspend execution,
//!    and resume on the same scheduler code was running on earlier.
//! 3. The receiver no longer has a scheduler.  
//!    Since the value signal now has a scheduler, we no longer need one.
//!    (Also, the C++ receivers have an optional scheduler, which makes coding against them really cumbersome.)
//! 4. Schedulers have a LocalScheduler type.  
//!    For most schedulers, that will be the scheduler itself.
//!    This allows for schedulers to declare follow-up code to run on a different scheduler.
//!    For example, an embarrasingly-parallel scheduler could schedule on any thread the first time,
//!    and then propagate a scheduler that'll always use the same thread for subsequent scheduling.
//! 5. Error/done recovery operations must complete with the same scheduler-type and value-type, as what would be sent during the happy path.  
//!    This is a consequence of us limiting things to a single type, and sending the scheduler along in the value-signal.
//!    (Note that the error and done signals don't propagate a scheduler alongside; this would be impossible without making [LetValue] a lot more cumbersome to use.)
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
mod upon_done;

pub use errors::{new_error, Error, IsTuple};
pub use just::Just;
pub use just_done::JustDone;
pub use just_error::JustError;
pub use let_value::LetValue;
pub use scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
pub use start_detached::start_detached;
pub use sync_wait::{sync_wait, sync_wait_send};
pub use then::Then;
pub use traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
pub use transfer::Transfer;
pub use upon_done::UponDone;
