use crate::embarrasingly_parallel::cross_thread_pool::CrossThreadPool;
use crate::embarrasingly_parallel::thread_local_pool::ThreadLocalPool;
use crate::embarrasingly_parallel::worker::Worker;
use crate::just_done::JustDone;
use crate::let_value::LetValue;
use crate::scheduler::{ImmediateScheduler, Scheduler};
use crate::scope::ScopeSend;
use crate::start_detached::start_detached;
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use rand::Rng;
use std::io;
use std::marker::PhantomData;
use std::ops::BitOr;
use std::sync::{Arc, Mutex};
use std::thread;

/// A threadpool with embarrasingly-parallel scheduling characteristics.
///
/// Submitted tasks will be scheduled on a randomly chosen thread,
/// and be invoked with a scheduler local to that thread.
/// This way you can run subsequent tasks on the same thread,
/// and actually achieve embarrasingly-parallel execution.
struct ThreadPoolState {
    threads: Vec<(CrossThreadPool, thread::JoinHandle<()>)>,
    pending_joins: Vec<thread::JoinHandle<()>>,
}

impl ThreadPoolState {
    /// Create a new threadpool state.
    ///
    /// The threadpool will have `thread_count` threads.
    /// Note that this type is the internal state, and should be shared with a reference-count-pointer, and a mutex.
    fn new(thread_count: usize) -> Result<Self, io::Error> {
        let mut pool = Self {
            threads: Vec::with_capacity(thread_count),
            pending_joins: Vec::new(),
        };
        pool.set_thread_count(thread_count)?;
        Ok(pool)
    }

    /// Change the number of threads that are part of the pool.
    ///
    /// If the number of threads is reduced, any active threads that are supposed to go away,
    /// will not go away until their last associated scheduler is closed and all tasks have run.
    /// However, those threads won't have new tasks scheduled on them anymore.
    /// This function won't wait for threads to complete.
    fn set_thread_count(&mut self, thread_count: usize) -> Result<usize, io::Error> {
        let old_size = self.threads.len();
        while self.threads.len() > thread_count {
            self.pending_joins.push(self.threads.pop().unwrap().1);
        }
        while self.threads.len() < thread_count {
            self.threads.push(Worker::start()?)
        }
        Ok(old_size)
    }

    /// Wait for all worker threads to complete.
    fn join(&mut self) {
        while let Some((_, h)) = self.threads.pop() {
            h.join().unwrap();
        }
        while let Some(h) = self.pending_joins.pop() {
            h.join().unwrap();
        }
    }
}

/// A threadpool with embarrasingly-parallel scheduling characteristics.
///
/// Submitted tasks will be scheduled on a randomly chosen thread,
/// and be invoked with a scheduler local to that thread.
/// This way you can run subsequent tasks on the same thread,
/// and actually achieve embarrasingly-parallel execution.
#[derive(Clone)]
pub struct ThreadPool {
    state: Arc<Mutex<ThreadPoolState>>,
}

impl ThreadPool {
    /// Create a new threadpool with the specified number of worker threads.
    pub fn new(thread_count: usize) -> Result<Self, io::Error> {
        Ok(Self {
            state: Arc::new(Mutex::new(ThreadPoolState::new(thread_count)?)),
        })
    }

    /// Join all threads.
    ///
    /// This function blocks until each of the threads has stopped.
    /// In order for a thread to stop, its scheduler has to no longer have any uses.
    ///
    /// Note that this function grabs the scheduler lock, meaning no other thread can schedule tasks until this function completes.
    pub fn join(self) {
        self.state.lock().unwrap().join();
    }

    /// Execute a task in the threadpool.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce(ThreadLocalPool) + Send + 'static,
    {
        start_detached(
            self.schedule()
                | LetValue::from(move |sch: ThreadLocalPool, _: ()| {
                    f(sch);
                    JustDone::<ImmediateScheduler, ()>::default()
                }),
        );
    }

    /// Run a task on each of the worker threads.
    pub fn execute_all<F>(&self, f: F)
    where
        F: FnOnce(ThreadLocalPool) + Clone + Send + 'static,
    {
        for i in &self.state.lock().unwrap().threads {
            let f = f.clone();
            start_detached(
                i.0.schedule()
                    | LetValue::from(move |sch: ThreadLocalPool, _: ()| {
                        f(sch);
                        JustDone::<ImmediateScheduler, ()>::default()
                    }),
            );
        }
    }

    /// Change the number of threads that are part of the pool.
    ///
    /// If the number of threads is reduced, any active threads that are supposed to go away,
    /// will not go away until their last associated scheduler is closed and all tasks have run.
    /// However, those threads won't have new tasks scheduled on them anymore.
    /// This function won't wait for threads to complete.
    pub fn set_thread_count(&mut self, thread_count: usize) -> Result<usize, io::Error> {
        self.state.lock().unwrap().set_thread_count(thread_count)
    }
}

impl Eq for ThreadPool {}

impl PartialEq<ThreadPool> for ThreadPool {
    fn eq(&self, rhs: &ThreadPool) -> bool {
        Arc::ptr_eq(&self.state, &rhs.state)
    }
}

impl Scheduler for ThreadPool {
    const EXECUTION_BLOCKS_CALLER: bool = false;
    type LocalScheduler = ThreadLocalPool;
    type Sender = ThreadPoolTS;

    fn schedule(&self) -> Self::Sender {
        Self::Sender { sch: self.clone() }
    }
}

pub struct ThreadPoolTS {
    sch: ThreadPool,
}

impl TypedSender<'_> for ThreadPoolTS {
    type Value = ();
    type Scheduler = ThreadLocalPool;
}

impl<'scope, 'a, ScopeImpl, ReceiverType> TypedSenderConnect<'scope, 'a, ScopeImpl, ReceiverType>
    for ThreadPoolTS
where
    'a: 'scope,
    ReceiverType: 'scope + ReceiverOf<ThreadLocalPool, ()> + Send,
    ScopeImpl: ScopeSend<'scope, 'a>,
{
    fn connect(self, scope: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope> {
        ThreadPoolOperationState {
            phantom: PhantomData,
            sch: self.sch,
            receiver: receiver,
            scope: scope.clone(),
        }
    }
}

impl<BindSenderImpl> BitOr<BindSenderImpl> for ThreadPoolTS
where
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ThreadPoolOperationState<'scope, 'a, ScopeImpl, ReceiverType>
where
    'a: 'scope,
    ReceiverType: ReceiverOf<ThreadLocalPool, ()> + Send + 'scope,
    ScopeImpl: ScopeSend<'scope, 'a>,
{
    phantom: PhantomData<(&'scope (), &'a ())>,
    sch: ThreadPool,
    receiver: ReceiverType,
    scope: ScopeImpl,
}

impl<'scope, 'a, ScopeImpl, ReceiverType> OperationState<'scope>
    for ThreadPoolOperationState<'scope, 'a, ScopeImpl, ReceiverType>
where
    'a: 'scope,
    ReceiverType: ReceiverOf<ThreadLocalPool, ()> + Send + 'scope,
    ScopeImpl: ScopeSend<'scope, 'a>,
{
    fn start(self) {
        let mut rng = rand::thread_rng();
        let state = self.sch.state.lock().unwrap();
        let thread_idx = rng.gen_range(0..state.threads.len());
        state.threads[thread_idx]
            .0
            .schedule()
            .connect(&self.scope, self.receiver)
            .start();
    }
}
