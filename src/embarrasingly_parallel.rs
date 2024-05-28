//! A scheduler that uses an embarrasingly-parallel strategy for executing tasks.
//!
//! ## Thread Pool Mode
//! To use a thread-pool, instantiate a [ThreadPool].
//! Submitting tasks on the [ThreadPool] will schedule them to run on an arbitrary thread.
//!
//! ## Manual Mode
//! To manually operate the scheduler, start by creating a [ThreadLocalPool] and associated [Worker].
//! Tasks will be run when you call [Worker::run] or [Worker::run_one].

mod cross_thread_pool;
mod pool;
mod tasks;
mod thread_local_pool;
mod worker;

pub use cross_thread_pool::CrossThreadPool;
pub use pool::ThreadPool;
pub use thread_local_pool::ThreadLocalPool;
pub use worker::Worker;
