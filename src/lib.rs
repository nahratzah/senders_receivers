#![crate_name = "senders_receivers"]
#![deny(missing_docs)]

//! # A Tiny Example
//! ```
//! use senders_receivers::*;
//!
//! let sender = Just::from((1, 2, 3, 4))
//!            | Then::from(|(a, b, c, d)| (a * b * c * d,));
//! println!("outcome: {}", sender.sync_wait().expect("no error").expect("no cancelation").0);
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
//! # How To Use This
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
//!
//! ## Composition
//! Multiple sender-chains can be merged together into a single sender-chain.
//! ```
//! use senders_receivers::{when_all, Just, Then, SyncWait};
//!
//! // The same chain from the previous example.
//! let sender_chain = Just::from((1, 2, 3))
//!                  | Then::from(|(x, y, z)| (x + y + z,));
//!
//! // Some other sender-chain, that computes a different value.
//! let another_sender_chain = Just::from((7, 8));
//!
//! let sender_chain = when_all!(
//!     sender_chain,
//!     another_sender_chain,
//! );
//!
//! let outcome = match sender_chain.sync_wait() {
//!     Ok(Some(values)) => values,
//!     Ok(None) => panic!("execution was canceled"),
//!     Err(error) => panic!("execution failed: {:?}", error),
//! };
//! assert_eq!(
//!     (6, 7, 8),
//!     outcome);
//! ```
//!
//! ## Schedulers
//! We can make the sender-chain run on a different scheduler.
//! For example, a threadpool.
//! [sync_wait_send](SyncWaitSend::sync_wait_send()) must be used, to allow completion across a thread boundary.
//! ```
//! use senders_receivers::{Scheduler, SyncWaitSend};
//! use threadpool::ThreadPool;
//!
//! let pool = ThreadPool::with_name("senders-receivers example".into(), 2);
//! let sender_chain = pool.schedule_value((1, 2, 3));
//! let outcome = match sender_chain.sync_wait_send() {
//!     Ok(Some(values)) => values,
//!     Ok(None) => panic!("execution was canceled"),
//!     Err(error) => panic!("execution failed: {:?}", error),
//! };
//! assert_eq!(
//!     (1, 2, 3),
//!     outcome);
//! ```
//!
//! We can even use temporary references.
//! ```
//! use senders_receivers::{Then, Scheduler, SyncWaitSend};
//! use threadpool::ThreadPool;
//!
//! let pool = ThreadPool::with_name("senders-receivers example".into(), 2);
//!
//! // We place the code in a scope, to show off that it handles lifetimes.
//! let (outcome,) = {
//!     let x = 6;
//!     let y = 7;
//!
//!     let sender = pool.schedule()
//!                | Then::from(|_| (x * y,));
//!     sender.sync_wait_send().unwrap().unwrap()
//! };
//!
//! assert_eq!(42, outcome);
//! ```
//!
//! We can also swap to a different scheduler part-way through a calculation, using the [Transfer] sender.
//! ```
//! use senders_receivers::{Just, Then, Transfer, SyncWaitSend};
//! use threadpool::ThreadPool;
//!
//! let pool = ThreadPool::with_name("senders-receivers example".into(), 2);
//!
//! let sender_chain = Just::default()
//!                  | Then::from(|_| (String::from("I run on the local thread"),))
//!                  | Transfer::new(pool)
//!                  | Then::from(|(previous,)| (previous, String::from("I run on the threadpool")));
//!
//! let outcome = match sender_chain.sync_wait_send() {
//!     Ok(Some(values)) => values,
//!     Ok(None) => panic!("execution was canceled"),
//!     Err(error) => panic!("execution failed: {:?}", error),
//! };
//! assert_eq!(
//!     (String::from("I run on the local thread"), String::from("I run on the threadpool")),
//!     outcome);
//! ```
//!
//! ## Combining Threadpool and Composition
//! We can combine the threadpool and composition, to run two or more tasks in parallel.
//!
//! This example will print out `first task`, `second task`, and `third task` in some unspecified order.
//! They will run on the thread-pool, which will place them on one of its worker threads.
//! ```
//! use senders_receivers::{when_all, Scheduler, Then, SyncWaitSend};
//! use threadpool::ThreadPool;
//!
//! let pool = ThreadPool::with_name("senders-receivers example".into(), 2);
//!
//! let first_sender_chain = pool.schedule_value(("first",))
//!                        | Then::from(|(x,)| {
//!                              println!("{} task", x);
//!                              (x,)
//!                          });
//! let second_sender_chain = pool.schedule_value(("second",))
//!                         | Then::from(|(x,)| {
//!                               println!("{} task", x);
//!                               (x,)
//!                           });
//! let third_sender_chain = pool.schedule_value(("third",))
//!                        | Then::from(|(x,)| {
//!                              println!("{} task", x);
//!                              (x,)
//!                          });
//! // Declare we want all three tasks to run as part of our sender-chain.
//! let sender_chain = when_all!(
//!     first_sender_chain,
//!     second_sender_chain,
//!     third_sender_chain,
//! );
//!
//! let outcome = match sender_chain.sync_wait_send() {
//!     Ok(Some(values)) => values,
//!     Ok(None) => panic!("execution was canceled"),
//!     Err(error) => panic!("execution failed: {:?}", error),
//! };
//! assert_eq!(
//!     ("first", "second", "third"),
//!     outcome);
//! ```

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
#[macro_use]
mod when_all_transfer;
mod let_done;
mod let_error;
mod split;

pub use errors::{new_error, Error, Result};
pub use just::Just;
pub use just_done::JustDone;
pub use just_error::JustError;
pub use let_done::LetDone;
pub use let_error::LetError;
pub use let_value::LetValue;
pub use scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
pub use split::{SharedError, Split, SplitSend};
pub use start_detached::StartDetached;
pub use sync_wait::{SyncWait, SyncWaitSend};
pub use then::Then;
pub use traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
};
pub use transfer::Transfer;
pub use upon_done::UponDone;
pub use upon_error::UponError;
#[doc(hidden)]
pub use when_all::WhenAll;
#[doc(hidden)]
pub use when_all_transfer::{
    NoSchedulerReceiver, NoSchedulerSender, NoSchedulerSenderImpl, NoSchedulerSenderValue,
    PairwiseTS, SchedulerTS,
};
