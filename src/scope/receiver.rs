use crate::errors::Error;
use crate::refs::ScopedRefMut;
use crate::scheduler::Scheduler;
use crate::scope::scope_data::ScopeData;
use crate::scope::ScopeImpl;
use crate::traits::{Receiver, ReceiverOf};
use crate::tuple::Tuple;
use std::cell::{OnceCell, RefCell};
use std::fmt;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

enum Argument<Sch, Values>
where
    Sch: Scheduler,
    Values: Tuple,
{
    ValueSignal(Sch, Values),
    ErrorSignal(Error),
    DoneSignal,
}

pub struct InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    inner_data: ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>,
    shared_value: SharedValueNoSend<Sch, Values>,
}

pub struct InnerScopeSendReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: Send + Sync + ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    inner_data: ScopeDataSendPtr<OuterState, Sch, Values, Rcv>,
    shared_value: SharedValueSend<Sch, Values>,
}

impl<OuterState, Sch, Values, Rcv> InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    pub(super) fn new<'inner_scope, 'outer_scope>(
        outer_scope: &ScopeImpl<'outer_scope, '_, OuterState>,
        rcv: Rcv,
    ) -> (
        ScopeImpl<'inner_scope, 'outer_scope, ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>>,
        Self,
        ScopedRefMut<
            'inner_scope,
            'outer_scope,
            Rcv,
            ScopeImpl<'inner_scope, 'outer_scope, ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>>,
        >,
    )
    where
        'outer_scope: 'inner_scope,
        Rcv: 'outer_scope,
    {
        let shared_value: SharedValueNoSend<Sch, Values> = Rc::new(RefCell::new(OnceCell::new()));
        let inner_data =
            ScopeDataNoSendPtr::new(outer_scope.data.clone(), shared_value.clone(), rcv);

        let inner_scope = ScopeImpl::new(inner_data.clone());
        let wrapper = Self {
            inner_data,
            shared_value,
        };
        let rcv_ref = {
            let mut scoped_data_ref = (*inner_scope.data.data).borrow_mut();
            let rcv: &mut Rcv = scoped_data_ref.rcv.get_mut().unwrap();
            let rcv = unsafe { mem::transmute::<&mut Rcv, &'inner_scope mut Rcv>(rcv) };
            ScopedRefMut::new(rcv, inner_scope.clone())
        };

        (inner_scope, wrapper, rcv_ref)
    }
}

impl<OuterState, Sch, Values, Rcv> InnerScopeSendReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: Send + Sync + ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    pub(super) fn new<'inner_scope, 'outer_scope>(
        outer_scope: &ScopeImpl<'outer_scope, '_, OuterState>,
        rcv: Rcv,
    ) -> (
        ScopeImpl<'inner_scope, 'outer_scope, ScopeDataSendPtr<OuterState, Sch, Values, Rcv>>,
        Self,
    )
    where
        'outer_scope: 'inner_scope,
        Rcv: 'outer_scope,
    {
        let shared_value: SharedValueSend<Sch, Values> = Arc::new(Mutex::from(None));
        let inner_data = ScopeDataSendPtr::new(outer_scope.data.clone(), shared_value.clone(), rcv);

        let inner_scope = ScopeImpl::new(inner_data.clone());
        let wrapper = Self {
            inner_data,
            shared_value,
        };

        (inner_scope, wrapper)
    }
}

impl<OuterState, Sch, Values, Rcv> Receiver for InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_error(self, error: Error) {
        if self
            .shared_value
            .borrow_mut()
            .set(Argument::ErrorSignal(error))
            .is_err()
        {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
    }

    fn set_done(self) {
        if self
            .shared_value
            .borrow_mut()
            .set(Argument::DoneSignal)
            .is_err()
        {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
    }
}

impl<OuterState, Sch, Values, Rcv> Receiver for InnerScopeSendReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: Send + Sync + ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_error(self, error: Error) {
        let mut opt_shared_value = self.shared_value.lock().unwrap();
        if opt_shared_value.is_some() {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
        let _ = opt_shared_value.insert(Argument::ErrorSignal(error));
    }

    fn set_done(self) {
        let mut opt_shared_value = self.shared_value.lock().unwrap();
        if opt_shared_value.is_some() {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
        let _ = opt_shared_value.insert(Argument::DoneSignal);
    }
}

impl<OuterState, Sch, Values, Rcv> ReceiverOf<Sch, Values>
    for InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_value(self, sch: Sch, values: Values) {
        if self
            .shared_value
            .borrow_mut()
            .set(Argument::ValueSignal(sch, values))
            .is_err()
        {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
    }
}

impl<OuterState, Sch, Values, Rcv> ReceiverOf<Sch, Values>
    for InnerScopeSendReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: Send + Sync + ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_value(self, sch: Sch, values: Values) {
        let mut opt_shared_value = self.shared_value.lock().unwrap();
        if opt_shared_value.is_some() {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
        let _ = opt_shared_value.insert(Argument::ValueSignal(sch, values));
    }
}

fn forward<Sch, Values, Rcv>(rcv: Rcv, argument: Option<Argument<Sch, Values>>)
where
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    match argument {
        Some(argument) => match argument {
            Argument::ValueSignal(sch, values) => rcv.set_value(sch, values),
            Argument::ErrorSignal(error) => rcv.set_error(error),
            Argument::DoneSignal => rcv.set_done(),
        },
        None => {}
    }
}

type SharedValueNoSend<Sch, Values> = Rc<RefCell<OnceCell<Argument<Sch, Values>>>>;
type SharedValueSend<Sch, Values> = Arc<Mutex<Option<Argument<Sch, Values>>>>;

struct ScopeDataNoSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    a_thread_panicked: bool,
    shared_value: SharedValueNoSend<Sch, Values>,
    outer_state: OuterState,
    rcv: OnceCell<Rcv>,
}

struct ScopeDataSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    a_thread_panicked: AtomicBool,
    shared_value: SharedValueSend<Sch, Values>,
    outer_state: OuterState,
    rcv: OnceLock<Rcv>,
}

impl<OuterState, Sch, Values, Rcv> ScopeDataNoSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn new(
        outer_state: OuterState,
        shared_value: SharedValueNoSend<Sch, Values>,
        rcv: Rcv,
    ) -> Self {
        Self {
            a_thread_panicked: false,
            shared_value,
            outer_state,
            rcv: OnceCell::from(rcv),
        }
    }

    fn mark_panicked(&mut self) {
        self.a_thread_panicked = true;
    }
}

impl<OuterState, Sch, Values, Rcv> ScopeDataSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn new(outer_state: OuterState, shared_value: SharedValueSend<Sch, Values>, rcv: Rcv) -> Self {
        Self {
            a_thread_panicked: AtomicBool::from(false),
            shared_value,
            outer_state,
            rcv: OnceLock::from(rcv),
        }
    }

    fn mark_panicked(&self) {
        self.a_thread_panicked.store(true, Ordering::Relaxed);
    }
}

impl<OuterState, Sch, Values, Rcv> Drop for ScopeDataNoSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn drop(&mut self) {
        match self.a_thread_panicked {
            true => self.outer_state.mark_panicked(),
            false => self.outer_state.run(|| {
                forward(
                    self.rcv
                        .take()
                        .expect("receiver has not been completed yet"),
                    self.shared_value.borrow_mut().take(),
                )
            }),
        }
    }
}

impl<OuterState, Sch, Values, Rcv> Drop for ScopeDataSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn drop(&mut self) {
        match self.a_thread_panicked.load(Ordering::Relaxed) {
            true => self.outer_state.mark_panicked(),
            false => self.outer_state.run(|| {
                forward(
                    self.rcv
                        .take()
                        .expect("receiver has not been completed yet"),
                    self.shared_value.lock().unwrap().take(),
                )
            }),
        }
    }
}

pub struct ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    data: Rc<RefCell<ScopeDataNoSend<OuterState, Sch, Values, Rcv>>>,
}

pub struct ScopeDataSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    data: Arc<ScopeDataSend<OuterState, Sch, Values, Rcv>>,
}

impl<OuterState, Sch, Values, Rcv> ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn new(
        outer_state: OuterState,
        shared_value: SharedValueNoSend<Sch, Values>,
        rcv: Rcv,
    ) -> Self {
        let data = Rc::new(RefCell::new(ScopeDataNoSend::new(
            outer_state,
            shared_value,
            rcv,
        )));
        Self { data }
    }
}

impl<OuterState, Sch, Values, Rcv> ScopeDataSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn new(outer_state: OuterState, shared_value: SharedValueSend<Sch, Values>, rcv: Rcv) -> Self {
        let data = Arc::new(ScopeDataSend::new(outer_state, shared_value, rcv));
        Self { data }
    }
}

impl<OuterState, Sch, Values, Rcv> ScopeData for ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn mark_panicked(&self) {
        self.data.borrow_mut().mark_panicked();
    }
}

impl<OuterState, Sch, Values, Rcv> ScopeData for ScopeDataSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn mark_panicked(&self) {
        self.data.mark_panicked();
    }
}

impl<OuterState, Sch, Values, Rcv> fmt::Debug for ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.borrow().fmt(f)
    }
}

impl<OuterState, Sch, Values, Rcv> fmt::Debug for ScopeDataSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}

impl<OuterState, Sch, Values, Rcv> Clone for ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<OuterState, Sch, Values, Rcv> Clone for ScopeDataSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<OuterState, Sch, Values, Rcv> fmt::Debug for ScopeDataNoSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We can't share the actual receiver, without imposing a Debug restriction on the user.
        // So we'll do the next best thing, which is to describe it.
        let rcv_description = match self.rcv.get() {
            Some(_) => "present",
            None => "absent",
        };

        // We can't share the actual value, without imposing a Debug restriction on the user.
        // So we'll do the next best thing, which is to describe it.
        let shared_value_description = match self.shared_value.borrow().get() {
            Some(argument_ref) => match argument_ref {
                Argument::ValueSignal(..) => "holding value signal",
                Argument::ErrorSignal(..) => "holding error signal",
                Argument::DoneSignal => "holding done signal",
            },
            None => "absent",
        };

        f.debug_struct("NestedScope")
            .field("a_thread_panicked", &self.a_thread_panicked)
            .field("outer_state", &self.outer_state)
            .field("rcv", &rcv_description)
            .field("shared_value", &shared_value_description)
            .finish()
    }
}

impl<OuterState, Sch, Values, Rcv> fmt::Debug for ScopeDataSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We can't share the actual receiver, without imposing a Debug restriction on the user.
        // So we'll do the next best thing, which is to describe it.
        let rcv_description = match self.rcv.get() {
            Some(_) => "present",
            None => "absent",
        };

        // We can't share the actual value, without imposing a Debug restriction on the user.
        // So we'll do the next best thing, which is to describe it.
        let shared_value_description = match self.shared_value.lock().unwrap().as_ref() {
            Some(argument_ref) => match argument_ref {
                Argument::ValueSignal(..) => "holding value signal",
                Argument::ErrorSignal(..) => "holding error signal",
                Argument::DoneSignal => "holding done signal",
            },
            None => "absent",
        };

        f.debug_struct("NestedScope")
            .field("a_thread_panicked", &self.a_thread_panicked)
            .field("outer_state", &self.outer_state)
            .field("rcv", &rcv_description)
            .field("shared_value", &shared_value_description)
            .finish()
    }
}
