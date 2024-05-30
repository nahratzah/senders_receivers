use crate::embarrasingly_parallel::cross_thread_pool::CrossThreadPool;
use crate::embarrasingly_parallel::tasks::{SendTask, Task};
use crate::embarrasingly_parallel::thread_local_pool::ThreadLocalPool;
use crate::sync::cross_thread_channel;
use crate::sync::same_thread_channel;
use mio::{Events, Poll, Token};
use std::io;
use std::thread;
use std::time::Duration;
const WAKEUP_TOKEN: Token = Token(usize::MAX);
const EVENTS_CAPACITY: usize = 1000;

#[derive(Debug)]
pub enum TryRecvError {
    Disconnected,
    Empty,
    IOError(io::Error),
}

impl From<cross_thread_channel::TryRecvError> for TryRecvError {
    fn from(error: cross_thread_channel::TryRecvError) -> Self {
        match error {
            cross_thread_channel::TryRecvError::Disconnected => Self::Disconnected,
            cross_thread_channel::TryRecvError::Empty => Self::Empty,
        }
    }
}

impl From<io::Error> for TryRecvError {
    fn from(error: io::Error) -> Self {
        Self::IOError(error)
    }
}

/// A worker for [ThreadLocalPool].
///
/// Usually, these workers are run in the background, by [ThreadPool](crate::embarrasingly_parallel::pool::ThreadPool).
/// But you can instantiate them manually, and run them yourself, should you choose so.
pub struct Worker {
    poll: Poll,
    events: Events,

    stc_sender: same_thread_channel::Sender<Task>,
    stc_recv: same_thread_channel::Receiver<Task>,
    xtc_recv: cross_thread_channel::Receiver<SendTask>,
}

impl Worker {
    /// Pick up any tasks posted by other threads, and schedule them.
    fn poll_move_send_tasks_to_tasks(&self) -> Result<(), TryRecvError> {
        loop {
            match self.xtc_recv.try_recv() {
                Ok(task) => self
                    .stc_sender
                    .send(task.into())
                    .expect("enqueue task should succeed"),
                Err(error) => {
                    return if self.stc_recv.is_empty() {
                        Err(error.into())
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }

    /// Poll the event queue.
    fn poll(&mut self, timeout: Option<Duration>) -> Result<(), TryRecvError> {
        self.poll.poll(&mut self.events, timeout)?;

        let mut result: Result<(), TryRecvError> = Ok(());
        for event in &self.events {
            if event.token() == WAKEUP_TOKEN {
                result = self.poll_move_send_tasks_to_tasks();
            } else {
                unimplemented!();
            }
        }
        result
    }

    /// Run a specific [Task].
    fn run_task(&self, task: Task) {
        task.invoke(self.stc_sender.clone())
    }

    /// Progress the worker, such that it runs one task.
    ///
    /// Returns `true` if a task was run, `false` otherwise.
    fn run_one_task(&mut self) -> bool {
        match self.stc_recv.try_recv() {
            Ok(task) => {
                self.run_task(task);
                true
            }
            Err(_) => false,
        }
    }

    /// Run the worker.
    ///
    /// Returns only once all tasks have been executed, and no more can be added.
    /// Note that if the calling thread has any [ThreadLocalPool] or [CrossThreadPool] corresponding to this worker,
    /// this function will never terminate.
    pub fn run(mut self) -> Result<(), io::Error> {
        loop {
            let poll_duration: Option<Duration> = if self.stc_recv.is_empty() {
                None
            } else {
                Some(Duration::from_secs(0))
            };

            match self.poll(poll_duration) {
                Ok(_) => {
                    let did_run_a_task = self.run_one_task(); // XXX change to drain
                    assert!(did_run_a_task);
                }
                Err(TryRecvError::IOError(error)) => return Err(error),
                Err(TryRecvError::Disconnected) => return Ok(()),
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    /// Progress the worker.
    ///
    /// Runs at most one task.
    /// If `block` is `true`, then the call will block until a task becomes available,
    /// or the last [ThreadLocalPool] and [CrossThreadPool] have all gone away.
    ///
    /// Returns `true` if a task was run, `false` otherwise.
    pub fn run_one(&mut self, block: bool) -> Result<bool, io::Error> {
        let poll_duration: Option<Duration> = if block && self.stc_recv.is_empty() {
            None
        } else {
            Some(Duration::from_secs(0))
        };

        match self.poll(poll_duration) {
            Ok(_) => {
                let did_run_a_task = self.run_one_task();
                assert!(did_run_a_task);
                Ok(did_run_a_task)
            }
            Err(TryRecvError::IOError(error)) => Err(error),
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => Ok(false),
        }
    }

    /// Create a new worker thread.
    ///
    /// Returns the [CrossThreadPool] that can post tasks to the thread,
    /// and a [JoinHandle](thread::JoinHandle) corresponding to the thread.
    ///
    /// The [Worker] is use internally by this thread.
    pub(super) fn start() -> Result<(CrossThreadPool, thread::JoinHandle<()>), io::Error> {
        let poll = Poll::new()?;
        let (xtc_sender, xtc_recv) = cross_thread_channel::channel(&poll, WAKEUP_TOKEN)?;
        let join_handle = thread::spawn(move || {
            let (stc_sender, stc_recv) = same_thread_channel::channel(1);
            let worker = Worker {
                poll,
                events: Events::with_capacity(EVENTS_CAPACITY),
                stc_sender,
                stc_recv,
                xtc_recv,
            };

            worker.run().expect("worker shouldn't fail");
        });
        let ctp = CrossThreadPool::new(join_handle.thread().id(), xtc_sender);

        Ok((ctp, join_handle))
    }

    /// Create a new worker.
    ///
    /// Returns the pool, and a scheduler corresponding to it.
    pub fn new() -> Result<(ThreadLocalPool, Worker), io::Error> {
        let poll = Poll::new()?;
        let (xtc_sender, xtc_recv) = cross_thread_channel::channel(&poll, WAKEUP_TOKEN)?;
        let (stc_sender, stc_recv) = same_thread_channel::channel(1);

        let worker = Worker {
            poll,
            events: Events::with_capacity(EVENTS_CAPACITY),
            stc_sender: stc_sender.clone(),
            stc_recv,
            xtc_recv,
        };
        let ctp = ThreadLocalPool::new(thread::current().id(), stc_sender, xtc_sender);

        Ok((ctp, worker))
    }
}
