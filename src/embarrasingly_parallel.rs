//! A scheduler that uses an embarrasingly-parallel strategy for executing tasks.
//!
//! ## Thread Pool Mode
//! To use a thread-pool, instantiate a [ThreadPool].
//! Submitting tasks on the [ThreadPool] will schedule them to run on an arbitrary thread.
//!
//! ```
//! use senders_receivers::embarrasingly_parallel::{ThreadPool, ThreadLocalPool};
//! use senders_receivers::{new_error, start_detached, Scheduler, Then};
//! use std::net::{TcpListener, TcpStream};
//! use std::io::Write;
//!
//! let pool = ThreadPool::new(4); // Create a pool with 4 worker threads.
//! let listener = TcpListener::bind("[::1]:0").unwrap();
//! listener.set_nonblocking(true).unwrap(); // stops the example from hanging
//!
//! for stream in listener.incoming() {
//!     match stream {
//!         Ok(stream) => {
//!             start_detached(
//!                 pool.schedule_value((stream,))
//!                 | Then::from(
//!                     |(mut stream,): (TcpStream,)| {
//!                         stream.write_all(b"hello world\n").map_err(new_error)
//!                     }))
//!         },
//!         Err(_) => break,
//!     }
//! }
//! ```
//!
//! ## Manual Mode
//! To manually operate the scheduler, start by creating a [ThreadLocalPool] and associated [Worker].
//! Tasks will be run when you call [Worker::run] or [Worker::run_one].
//!
//! ```
//! use senders_receivers::embarrasingly_parallel::Worker;
//! use senders_receivers::{start_detached, Scheduler, Then};
//! use std::thread;
//!
//! fn print_number((i,): (i32,)) {
//!     println!("task {} running in {:?}\n", i, thread::current().id());
//! }
//!
//! let (pool, worker) = Worker::new();
//! for i in 0..10 {
//!     start_detached(pool.schedule_value((i,)) | Then::from(print_number));
//! }
//! drop(pool); // Without this, the worker.run() function will never complete.
//!
//! worker.run(); // Run all the tasks.
//! ```

mod cross_thread_pool;
mod pool;
mod tasks;
mod thread_local_pool;
mod worker;

pub use cross_thread_pool::CrossThreadPool;
pub use pool::ThreadPool;
pub use thread_local_pool::ThreadLocalPool;
pub use worker::Worker;
