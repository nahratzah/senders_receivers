use crate::embarrasingly_parallel::cross_thread_pool::CrossThreadPool;
use crate::embarrasingly_parallel::tasks::{SendTask, Task};
use crate::scheduler::Scheduler;
use crate::scope::ScopeWrap;
use crate::sync::{cross_thread_channel, same_thread_channel};
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use std::marker::PhantomData;
use std::ops::BitOr;
use std::thread;

/// A [Scheduler] that will run tasks on the current thread.
#[derive(Clone)]
pub struct ThreadLocalPool {
    thread_id: thread::ThreadId,
    stc_sender: same_thread_channel::Sender<Task>,
    xtc_sender: cross_thread_channel::Sender<SendTask>,
}

impl ThreadLocalPool {
    /// Create a new thread-local-pool.
    pub(super) fn new(
        thread_id: thread::ThreadId,
        stc_sender: same_thread_channel::Sender<Task>,
        xtc_sender: cross_thread_channel::Sender<SendTask>,
    ) -> Self {
        Self {
            thread_id,
            stc_sender,
            xtc_sender,
        }
    }

    /// Unpack the pool into parts.
    /// Used during conversion to [CrossThreadPool].
    pub(super) fn unpack(self) -> (thread::ThreadId, cross_thread_channel::Sender<SendTask>) {
        (self.thread_id, self.xtc_sender)
    }

    /// Execute a specific function on this pool.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce(ThreadLocalPool) + Sized + 'static,
    {
        self.stc_sender
            .send(Task::new(
                self.thread_id,
                self.xtc_sender.clone(),
                Box::new(f),
            ))
            .expect("receiver remains active");
    }

    /// Returns the [ThreadId](thread::ThreadId) of the pool.
    pub fn thread_id(&self) -> thread::ThreadId {
        self.thread_id
    }
}

impl Eq for ThreadLocalPool {}

impl PartialEq<ThreadLocalPool> for ThreadLocalPool {
    fn eq(&self, rhs: &ThreadLocalPool) -> bool {
        self.thread_id == rhs.thread_id
    }
}

impl PartialEq<CrossThreadPool> for ThreadLocalPool {
    fn eq(&self, rhs: &CrossThreadPool) -> bool {
        self.thread_id == rhs.thread_id()
    }
}

impl Scheduler for ThreadLocalPool {
    const EXECUTION_BLOCKS_CALLER: bool = false;
    type LocalScheduler = Self;
    type Sender = ThreadLocalPoolTS;

    fn schedule(&self) -> Self::Sender {
        Self::Sender { sch: self.clone() }
    }
}

pub struct ThreadLocalPoolTS {
    sch: ThreadLocalPool,
}

impl TypedSender for ThreadLocalPoolTS {
    type Scheduler = ThreadLocalPool;
    type Value = ();
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType> for ThreadLocalPoolTS
where
    ReceiverType: ReceiverOf<ThreadLocalPool, ()>,
    ScopeImpl: ScopeWrap<ThreadLocalPool, ReceiverType>,
{
    type Output<'scope> = ThreadLocalPoolOperationState<'scope, ScopeImpl::WrapOutput>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        _: StopTokenImpl,
        receiver: ReceiverType,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        ThreadLocalPoolOperationState {
            sch: self.sch,
            receiver: scope.wrap(receiver),
            phantom: PhantomData,
        }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for ThreadLocalPoolTS
where
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ThreadLocalPoolOperationState<'scope, ReceiverType>
where
    ReceiverType: ReceiverOf<ThreadLocalPool, ()> + 'static,
{
    phantom: PhantomData<&'scope ()>,
    sch: ThreadLocalPool,
    receiver: ReceiverType,
}

impl<'scope, ReceiverType> OperationState<'scope>
    for ThreadLocalPoolOperationState<'scope, ReceiverType>
where
    ReceiverType: ReceiverOf<ThreadLocalPool, ()> + 'static,
{
    fn start(self) {
        let receiver = self.receiver;
        self.sch.execute(move |sch: ThreadLocalPool| {
            receiver.set_value(sch, ());
        })
    }
}
