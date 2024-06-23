use crate::embarrasingly_parallel::tasks::SendTask;
use crate::embarrasingly_parallel::thread_local_pool::ThreadLocalPool;
use crate::scheduler::Scheduler;
use crate::sync::cross_thread_channel;

use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use std::ops::BitOr;
use std::thread;

/// Cross-thread counterpart of [ThreadLocalPool].
///
/// This scheduler can be moved across threads.
/// Tasks scheduled on it, will be run on the thread of the corresponding [ThreadLocalPool].
#[derive(Clone)]
pub struct CrossThreadPool {
    thread_id: thread::ThreadId,
    xtc_sender: cross_thread_channel::Sender<SendTask>,
}

impl CrossThreadPool {
    /// Create a new cross-thread-pool.
    pub(super) fn new(
        thread_id: thread::ThreadId,
        xtc_sender: cross_thread_channel::Sender<SendTask>,
    ) -> Self {
        Self {
            thread_id,
            xtc_sender,
        }
    }

    /// Execute a specific function on this pool.
    /// It'll run in the thread of the associated [ThreadLocalPool].
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce(ThreadLocalPool) + Sized + Send + 'static,
    {
        self.xtc_sender
            .send(SendTask::new(
                self.thread_id,
                self.xtc_sender.clone(),
                Box::new(f),
            ))
            .expect("receiver remains active");
    }

    /// Return the [ThreadId](thread::ThreadId) of the pool.
    pub fn thread_id(&self) -> thread::ThreadId {
        self.thread_id
    }
}

impl Eq for CrossThreadPool {}

impl PartialEq<CrossThreadPool> for CrossThreadPool {
    fn eq(&self, rhs: &CrossThreadPool) -> bool {
        self.thread_id == rhs.thread_id
    }
}

impl PartialEq<ThreadLocalPool> for CrossThreadPool {
    fn eq(&self, rhs: &ThreadLocalPool) -> bool {
        self.thread_id == rhs.thread_id()
    }
}

impl From<ThreadLocalPool> for CrossThreadPool {
    fn from(tlp: ThreadLocalPool) -> Self {
        let (thread_id, xtc_sender) = tlp.unpack();
        Self {
            thread_id,
            xtc_sender,
        }
    }
}

impl Scheduler for CrossThreadPool {
    const EXECUTION_BLOCKS_CALLER: bool = false;
    type LocalScheduler = ThreadLocalPool;
    type Sender = CrossThreadPoolTS;

    fn schedule(&self) -> Self::Sender {
        Self::Sender { sch: self.clone() }
    }
}

pub struct CrossThreadPoolTS {
    sch: CrossThreadPool,
}

impl<'a> TypedSender<'a> for CrossThreadPoolTS {
    type Scheduler = ThreadLocalPool;
    type Value = ();
}

impl<'a, ReceiverType> TypedSenderConnect<'a, ReceiverType> for CrossThreadPoolTS
where
    ReceiverType: ReceiverOf<ThreadLocalPool, ()> + Send + 'static,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        CrossThreadPoolOperationState {
            sch: self.sch,
            receiver,
        }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for CrossThreadPoolTS
where
    BindSenderImpl: BindSender<Self> + Send + 'static,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct CrossThreadPoolOperationState<ReceiverType>
where
    ReceiverType: ReceiverOf<ThreadLocalPool, ()> + Send + 'static,
{
    sch: CrossThreadPool,
    receiver: ReceiverType,
}

impl<ReceiverType> OperationState for CrossThreadPoolOperationState<ReceiverType>
where
    ReceiverType: ReceiverOf<ThreadLocalPool, ()> + Send + 'static,
{
    fn start(self) {
        let receiver = self.receiver;
        self.sch.execute(move |sch: ThreadLocalPool| {
            receiver.set_value(sch, ());
        })
    }
}
