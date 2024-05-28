use crate::embarrasingly_parallel::cross_thread_pool::CrossThreadPool;
use crate::embarrasingly_parallel::tasks::{SendTask, Task};
use crate::embarrasingly_parallel::thread_local_pool::ThreadLocalPool;

use crate::sync_wait::same_thread_channel;

use std::sync::mpsc;
use std::thread;

/// A worker for [ThreadLocalPool].
///
/// Usually, these workers are run in the background, by [ThreadPool](crate::embarrasingly_parallel::pool::ThreadPool).
/// But you can instantiate them manually, and run them yourself, should you choose so.
pub struct Worker {
    stc_sender: same_thread_channel::Sender<Task>,
    stc_recv: same_thread_channel::Receiver<Task>,
    xtc_recv: mpsc::Receiver<SendTask>,
}

impl Worker {
    /// Pick up any tasks posted by other threads, and schedule them.
    fn move_send_tasks_to_tasks(&mut self) {
        while let Ok(task) = self.xtc_recv.try_recv() {
            self.stc_sender
                .send(task.into())
                .expect("enqueue task should succeed");
        }
    }

    /// Pick up any tasks posted by other threads, and schedule them.
    /// If there are no tasks available, this will block.
    fn block_and_move_send_tasks_to_tasks(&mut self) -> bool {
        match self.xtc_recv.recv() {
            Ok(task) => self
                .stc_sender
                .send(task.into())
                .expect("enqueue task should succeed"),
            Err(_) => return false,
        }
        self.move_send_tasks_to_tasks();
        true
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
    pub fn run(mut self) {
        loop {
            if self.run_one_task() {
                self.move_send_tasks_to_tasks();
            } else if !self.block_and_move_send_tasks_to_tasks() {
                break;
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
    pub fn run_one(&mut self, block: bool) -> bool {
        self.move_send_tasks_to_tasks();
        if self.run_one_task() {
            true
        } else if block && self.block_and_move_send_tasks_to_tasks() {
            self.run_one_task()
        } else {
            false
        }
    }

    /// Create a new worker thread.
    ///
    /// Returns the [CrossThreadPool] that can post tasks to the thread,
    /// and a [JoinHandle](thread::JoinHandle) corresponding to the thread.
    ///
    /// The [Worker] is use internally by this thread.
    pub(super) fn start() -> (CrossThreadPool, thread::JoinHandle<()>) {
        let (xtc_sender, xtc_recv) = mpsc::channel();
        let join_handle = thread::spawn(move || {
            let (stc_sender, stc_recv) = same_thread_channel::channel(1);
            let worker = Worker {
                stc_sender,
                stc_recv,
                xtc_recv,
            };

            worker.run();
        });
        let ctp = CrossThreadPool::new(join_handle.thread().id(), xtc_sender);

        (ctp, join_handle)
    }

    /// Create a new worker.
    ///
    /// Returns the pool, and a scheduler corresponding to it.
    pub fn new() -> (ThreadLocalPool, Worker) {
        let (xtc_sender, xtc_recv) = mpsc::channel();
        let (stc_sender, stc_recv) = same_thread_channel::channel(1);

        let worker = Worker {
            stc_sender: stc_sender.clone(),
            stc_recv,
            xtc_recv,
        };
        let ctp = ThreadLocalPool::new(thread::current().id(), stc_sender, xtc_sender);

        (ctp, worker)
    }
}
