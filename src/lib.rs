#![crate_name = "senders_receivers"]
#![deny(missing_docs)]

//! # A Tiny Example
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
//! # Signals
//! Each sender, produces either
//! a `value signal`,
//! an `error signal`,
//! or a `done signal`.
//! Exactly one of these will be produced.
//!
//! A `value signal` indicates that the sender completed successfully, and produced a value.  
//! An `error signal` indicates that the sender failed, and produced an [Error].  
//! A `done signal` indicates that the sender canceled its work (producing neither a value, nor an error).
//!
//! # Schedulers
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
//! # Internals
//! The system works by creating sender-chains, which consist of a sequence of senders.
//!
//! ## Initial Element of the Sender-Chain
//! The first element in a sender-chain is a [TypedSender], which produces a value of some kind.
//! Usually, this will be a [Just] or a [Scheduler::schedule_value].
//! The inital element produces a value, which is a tuple.
//!
//! Example:
//! ```
//! use senders_receivers::Just;
//!
//! let sender_chain = Just::from((1, 2, 3));
//! ```
//!
//! To get the value produced by a sender-chain, you can use [SyncWait::sync_wait].
//! (This function will block until the sender-chain complets.)
//! ```
//! use senders_receivers::{Just, SyncWait};
//!
//! let sender_chain = Just::from((1, 2, 3));
//! let outcome = match sender_chain.sync_wait() {
//!     Ok(Some(values)) => values,
//!     Ok(None) => panic!("execution was canceled"),
//!     Err(error) => panic!("execution failed: {:?}", error),
//! };
//! assert_eq!(
//!     (1, 2, 3),
//!     outcome);
//! ```
//!
//! The [SyncWait::sync_wait] method returns a `Result<Option< value-type >>`, so we need two unwraps.
//!
//! ## Making the Sender-Chain do Actual Work
//! The above sender-chain is not very useful.
//! But we can make the sender-chain do some work for us.
//! For example, we can use [Then] to run some computation.
//! To attach a sender, we use the `|` (pipe) symbol.
//! ```
//! use senders_receivers::{Just, Then, SyncWait};
//!
//! let sender_chain = Just::from((1, 2, 3));
//!
//! // Declare we want to do a thing.
//! let sender_chain = sender_chain
//!                  | Then::from(|(x, y, z)| (x + y + z,));
//!
//! // The code in `Then` doesn't run until we call `sync_wait`.
//! let outcome = match sender_chain.sync_wait() {
//!     Ok(Some(values)) => values,
//!     Ok(None) => panic!("execution was canceled"),
//!     Err(error) => panic!("execution failed: {:?}", error),
//! };
//! assert_eq!(
//!     (6,),
//!     outcome);
//! ```
//!
//! [Then] is a [Sender].
//! That means it can be added to a sender-chain, and the addition produces a new sender-chain.
//! Since attaching a sender to a sender-chain results in a new sender-chain, you can keep doing this, creating more complex chains.
//!
//! The operations on the sender-chain won't run, until the sender is started using [sync_wait](SyncWait::sync_wait()).

pub mod embarrasingly_parallel;
mod errors;
pub mod functor;
pub mod io;
mod just;
mod just_done;
mod just_error;
mod let_value;
pub mod refs;
mod scheduler;
mod scope;
mod start_detached;
mod sync;
mod sync_wait;
mod then;
mod traits;
mod transfer;
pub mod tuple;
mod upon_done;
mod upon_error;
#[macro_use]
mod when_all;

pub use errors::{new_error, Error, Result};
pub use just::Just;
pub use just_done::JustDone;
pub use just_error::JustError;
pub use let_value::LetValue;
pub use scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
pub use start_detached::start_detached;
pub use sync_wait::{sync_wait, sync_wait_send, SyncWait, SyncWaitSend};
pub use then::Then;
pub use traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
pub use transfer::Transfer;
pub use upon_done::UponDone;
pub use upon_error::UponError;
pub use when_all::WhenAll;
