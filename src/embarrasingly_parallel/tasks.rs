use crate::embarrasingly_parallel::thread_local_pool::ThreadLocalPool;
use crate::sync::same_thread_channel;
use std::sync::mpsc;
use std::thread;

/// A task that can run on the ThreadLocalPool.
///
/// This type is not sendable, and thus allows for using things like [Rc](std::rc::Rc).
pub struct Task {
    /// Thread ID of the thread on which this is supposed to run.
    thread_id: thread::ThreadId,
    /// Cross-Thread-Channel sender.
    ///
    /// We use this to recreate a [ThreadLocalPool] when the task is invoked.
    /// It also is how the [ThreadLocalPool] is kept live.
    xtc_sender: mpsc::Sender<SendTask>,
    /// Implementation function.
    function: Box<dyn FnOnce(ThreadLocalPool) + 'static>,
}

impl Task {
    /// Create a new task.
    pub fn new(
        thread_id: thread::ThreadId,
        xtc_sender: mpsc::Sender<SendTask>,
        function: Box<dyn FnOnce(ThreadLocalPool) + 'static>,
    ) -> Self {
        Self {
            thread_id,
            xtc_sender,
            function,
        }
    }

    /// Run the task.
    ///
    /// Note that the [SendTask] doesn't have its own implementation, since a [SendTask] is only used
    /// when the task is on a different thread, and thus not allowed to run.
    pub fn invoke(self, stc_sender: same_thread_channel::Sender<Task>) {
        assert_eq!(
            self.thread_id,
            thread::current().id(),
            "invocation should happen on the worker thread"
        );
        (self.function)(ThreadLocalPool::new(
            self.thread_id,
            stc_sender,
            self.xtc_sender,
        ));
    }
}

impl From<SendTask> for Task {
    fn from(task: SendTask) -> Task {
        Task {
            thread_id: task.thread_id,
            xtc_sender: task.xtc_sender,
            function: task.function,
        }
    }
}

/// Similar to [Task], except sendable.
///
/// Due to being sendable, it requires the function to be sendable too.
pub struct SendTask {
    /// Thread ID of the thread on which this is supposed to run.
    thread_id: thread::ThreadId,
    /// Cross-Thread-Channel sender.
    ///
    /// We use this to recreate a [ThreadLocalPool] when the task is invoked.
    /// It also is how the [ThreadLocalPool] is kept live.
    xtc_sender: mpsc::Sender<SendTask>,
    /// Implementation function.
    function: Box<dyn FnOnce(ThreadLocalPool) + Send + 'static>,
}

impl SendTask {
    /// Create a new task.
    pub fn new(
        thread_id: thread::ThreadId,
        xtc_sender: mpsc::Sender<SendTask>,
        function: Box<dyn FnOnce(ThreadLocalPool) + Send + 'static>,
    ) -> Self {
        Self {
            thread_id,
            xtc_sender,
            function,
        }
    }
}
