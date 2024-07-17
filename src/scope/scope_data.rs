use super::receiver;
use crate::refs::ScopedRefMut;
use crate::scheduler::Scheduler;
use crate::traits::ReceiverOf;
use crate::tuple::Tuple;
use std::cell::RefCell;
use std::fmt;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub trait ScopeData: Clone + fmt::Debug {
    type NewScopeType<Sch, Values, ReceiverType>: ScopeData
    where
        Sch: Scheduler,
        Values: Tuple,
        ReceiverType: ReceiverOf<Sch, Values>;

    type NewReceiver<Sch, Values, ReceiverType>: ReceiverOf<Sch, Values>
    where
        Sch: Scheduler,
        Values: Tuple,
        ReceiverType: ReceiverOf<Sch, Values>;

    fn new_scope<Sch, Values, ReceiverType>(
        &self,
        rcv: ReceiverType,
    ) -> (
        Self::NewScopeType<Sch, Values, ReceiverType>,
        Self::NewReceiver<Sch, Values, ReceiverType>,
        ScopedRefMut<ReceiverType, Self::NewScopeType<Sch, Values, ReceiverType>>,
    )
    where
        Sch: Scheduler,
        Values: Tuple,
        ReceiverType: ReceiverOf<Sch, Values>;

    fn run<F, T>(&self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(result) => result,
            Err(unwind) => {
                self.mark_panicked();
                resume_unwind(unwind)
            }
        }
    }

    fn mark_panicked(&self);
}

pub(super) trait ScopeDataState {
    fn running(&self) -> bool;
    fn a_thread_panicked(&self) -> bool;
}

struct ScopeDataSend {
    num_running_threads: AtomicUsize,
    a_thread_panicked: AtomicBool,
    notify: Mutex<Option<Box<dyn Send + Sync + FnOnce(&dyn ScopeDataState)>>>,
}

impl ScopeDataSend {
    fn increment_num_running_threads(&self) {
        if self.num_running_threads.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            self.overflow();
        }
    }

    #[cold]
    fn overflow(&self) {
        self.decrement_num_running_threads();
        panic!("too many running threads in thread scope");
    }

    fn mark_panicked(&self) {
        self.a_thread_panicked.store(true, Ordering::Relaxed);
    }

    fn decrement_num_running_threads(&self) {
        if self.num_running_threads.fetch_sub(1, Ordering::Release) == 1 {
            (self
                .notify
                .lock()
                .unwrap()
                .take()
                .expect("has not been notified before"))(self);
        }
    }
}

impl ScopeDataState for ScopeDataSend {
    fn running(&self) -> bool {
        self.num_running_threads.load(Ordering::Acquire) != 0
    }

    fn a_thread_panicked(&self) -> bool {
        self.a_thread_panicked.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for ScopeDataSend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scope")
            .field(
                "num_running_threads",
                &self.num_running_threads.load(Ordering::Relaxed),
            )
            .field(
                "a_thread_panicked",
                &self.a_thread_panicked.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

struct ScopeDataNoSend {
    num_running_threads: usize,
    a_thread_panicked: bool,
    notify: Option<Box<dyn FnOnce(&dyn ScopeDataState)>>,
}

impl ScopeDataNoSend {
    fn increment_num_running_threads(&mut self) {
        if self.num_running_threads > usize::MAX / 2 {
            self.overflow();
        } else {
            self.num_running_threads += 1;
        }
    }

    #[cold]
    fn overflow(&mut self) {
        self.decrement_num_running_threads();
        panic!("too many running threads in thread scope");
    }

    fn mark_panicked(&mut self) {
        self.a_thread_panicked = true;
    }

    fn decrement_num_running_threads(&mut self) {
        self.num_running_threads -= 1;
        if self.num_running_threads == 0 {
            (self.notify.take().expect("has not been notified before"))(self);
        }
    }
}

impl ScopeDataState for ScopeDataNoSend {
    fn running(&self) -> bool {
        self.num_running_threads != 0
    }

    fn a_thread_panicked(&self) -> bool {
        self.a_thread_panicked
    }
}

impl fmt::Debug for ScopeDataNoSend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scope")
            .field("num_running_threads", &self.num_running_threads)
            .field("a_thread_panicked", &self.a_thread_panicked)
            .finish_non_exhaustive()
    }
}

/// A smart pointer which keeps a [ScopeDataSend] live.
/// Once the last pointer gets dropped, the notify method will be run.
#[derive(Debug)]
pub struct ScopeDataSendPtr {
    data: Arc<ScopeDataSend>,
}

/// Counterpart of a [ScopeDataSendPtr].
#[derive(Debug, Clone)]
pub(super) struct ScopeDataSendState {
    data: Arc<ScopeDataSend>,
}

impl ScopeDataSendPtr {
    pub(super) fn new(
        notify: impl 'static + Send + Sync + FnOnce(&dyn ScopeDataState),
    ) -> (ScopeDataSendState, Self) {
        let data = Arc::new(ScopeDataSend {
            num_running_threads: AtomicUsize::new(1),
            a_thread_panicked: AtomicBool::new(false),
            notify: Mutex::new(Some(Box::new(notify))),
        });
        let state_data = data.clone();
        (ScopeDataSendState { data: state_data }, Self { data })
    }
}

impl ScopeData for ScopeDataSendPtr {
    type NewScopeType<NestedSch, NestedValues, NestedRcv> = receiver::ScopeDataSendPtr<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;
    type NewReceiver<NestedSch, NestedValues, NestedRcv> = receiver::InnerScopeSendReceiver<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;

    fn new_scope<NestedSch, NestedValues, NestedRcv>(
        &self,
        rcv: NestedRcv,
    ) -> (
        Self::NewScopeType<NestedSch, NestedValues, NestedRcv>,
        Self::NewReceiver<NestedSch, NestedValues, NestedRcv>,
        ScopedRefMut<NestedRcv, Self::NewScopeType<NestedSch, NestedValues, NestedRcv>>,
    )
    where
        NestedSch: Scheduler,
        NestedValues: Tuple,
        NestedRcv: ReceiverOf<NestedSch, NestedValues>,
    {
        Self::NewReceiver::<NestedSch, NestedValues, NestedRcv>::new(self, rcv)
    }

    fn mark_panicked(&self) {
        self.data.mark_panicked()
    }
}

impl ScopeDataState for ScopeDataSendState {
    fn running(&self) -> bool {
        self.data.running()
    }

    fn a_thread_panicked(&self) -> bool {
        self.data.a_thread_panicked()
    }
}

impl Drop for ScopeDataSendPtr {
    fn drop(&mut self) {
        self.data.decrement_num_running_threads();
    }
}

impl Clone for ScopeDataSendPtr {
    fn clone(&self) -> Self {
        self.data.increment_num_running_threads();
        Self {
            data: self.data.clone(),
        }
    }
}

/// A smart pointer which keeps a [ScopeDataNoSend] live.
/// Once the last pointer gets dropped, the notify method will be run.
#[derive(Debug)]
pub struct ScopeDataPtr {
    data: Rc<RefCell<ScopeDataNoSend>>,
}

/// Counterpart of a [ScopeDataPtr].
#[derive(Debug, Clone)]
pub(super) struct ScopeDataNoSendState {
    data: Rc<RefCell<ScopeDataNoSend>>,
}

impl ScopeDataPtr {
    pub(super) fn new(
        notify: impl 'static + FnOnce(&dyn ScopeDataState),
    ) -> (ScopeDataNoSendState, Self) {
        let data = Rc::new(RefCell::new(ScopeDataNoSend {
            num_running_threads: 1,
            a_thread_panicked: false,
            notify: Some(Box::new(notify)),
        }));
        let state_data = data.clone();
        (ScopeDataNoSendState { data: state_data }, Self { data })
    }
}

impl ScopeData for ScopeDataPtr {
    type NewScopeType<NestedSch, NestedValues, NestedRcv> = receiver::ScopeDataNoSendPtr<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;
    type NewReceiver<NestedSch, NestedValues, NestedRcv> = receiver::InnerScopeReceiver<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;

    fn new_scope<NestedSch, NestedValues, NestedRcv>(
        &self,
        rcv: NestedRcv,
    ) -> (
        Self::NewScopeType<NestedSch, NestedValues, NestedRcv>,
        Self::NewReceiver<NestedSch, NestedValues, NestedRcv>,
        ScopedRefMut<NestedRcv, Self::NewScopeType<NestedSch, NestedValues, NestedRcv>>,
    )
    where
        NestedSch: Scheduler,
        NestedValues: Tuple,
        NestedRcv: ReceiverOf<NestedSch, NestedValues>,
    {
        Self::NewReceiver::<NestedSch, NestedValues, NestedRcv>::new(self, rcv)
    }

    fn mark_panicked(&self) {
        self.data.borrow_mut().mark_panicked();
    }
}

impl ScopeDataState for ScopeDataNoSendState {
    fn running(&self) -> bool {
        self.data.borrow().running()
    }

    fn a_thread_panicked(&self) -> bool {
        self.data.borrow().a_thread_panicked()
    }
}

impl Drop for ScopeDataPtr {
    fn drop(&mut self) {
        self.data.borrow_mut().decrement_num_running_threads();
    }
}

impl Clone for ScopeDataPtr {
    fn clone(&self) -> Self {
        self.data.borrow_mut().increment_num_running_threads();
        Self {
            data: self.data.clone(),
        }
    }
}
