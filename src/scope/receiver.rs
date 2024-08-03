use crate::errors::Error;
use crate::refs::ScopedRefMut;
use crate::scheduler::Scheduler;
use crate::scope::scope_data::ScopeData;
use crate::traits::{Receiver, ReceiverOf};
use crate::tuple::Tuple;
use std::cell::{OnceCell, RefCell};
use std::fmt;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

enum Signal<Sch, Values>
where
    Sch: Scheduler,
    Values: Tuple,
{
    Value(Sch, Values),
    Error(Error),
    Done,
}

pub(super) trait InnerScopeConstructor<OuterState, Rcv>: Sized {
    type ScopeDataPtr: Clone + fmt::Debug;

    fn new(
        outer_scope: &OuterState,
        rcv: Rcv,
    ) -> (
        Self::ScopeDataPtr,
        Self,
        ScopedRefMut<Rcv, Self::ScopeDataPtr>,
    );
}

pub struct InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    inner_data: ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>,
    shared_value: SharedValueNoSend<Sch, Values>,
}

pub struct InnerScopeSendReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    inner_data: ScopeDataSendPtr<OuterState, Sch, Values, Rcv>,
    shared_value: SharedValueSend<Sch, Values>,
}

impl<OuterState, Sch, Values, Rcv> InnerScopeConstructor<OuterState, Rcv>
    for InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    type ScopeDataPtr = ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>;

    fn new(
        outer_scope: &OuterState,
        rcv: Rcv,
    ) -> (
        Self::ScopeDataPtr,
        Self,
        ScopedRefMut<Rcv, Self::ScopeDataPtr>,
    ) {
        let shared_value: SharedValueNoSend<Sch, Values> = Rc::new(RefCell::new(OnceCell::new()));
        let inner_data = ScopeDataNoSendPtr::new(outer_scope.clone(), shared_value.clone(), rcv);

        let rcv_ref = {
            let mut scoped_data_ref = (*inner_data.data).borrow_mut();
            let rcv: &mut Rcv = scoped_data_ref.rcv.get_mut().unwrap();
            let rcv = unsafe { mem::transmute::<&mut Rcv, &mut Rcv>(rcv) };
            unsafe { ScopedRefMut::new(rcv, inner_data.clone()) } // rcv is part of inner_data, so we guarantee the lifetime
        };
        let wrapper = InnerScopeReceiver {
            inner_data: inner_data.clone(),
            shared_value,
        };

        (inner_data, wrapper, rcv_ref)
    }
}

impl<OuterState, Sch, Values, Rcv> InnerScopeConstructor<OuterState, Rcv>
    for InnerScopeSendReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    type ScopeDataPtr = ScopeDataSendPtr<OuterState, Sch, Values, Rcv>;

    fn new(
        outer_scope: &OuterState,
        rcv: Rcv,
    ) -> (
        Self::ScopeDataPtr,
        Self,
        ScopedRefMut<Rcv, Self::ScopeDataPtr>,
    ) {
        let shared_value: SharedValueSend<Sch, Values> = Arc::new(Mutex::from(None));
        let mut inner_data = ScopeDataSendPtr::new(outer_scope.clone(), shared_value.clone(), rcv);

        let rcv_ref = {
            let rcv: &mut Rcv = Arc::get_mut(&mut inner_data.data)
                .unwrap()
                .rcv
                .get_mut()
                .unwrap();
            let rcv = unsafe { mem::transmute::<&mut Rcv, &mut Rcv>(rcv) };
            unsafe { ScopedRefMut::new(rcv, inner_data.clone()) } // rcv is part of inner_data, so we guarantee the lifetime
        };
        let wrapper = InnerScopeSendReceiver {
            inner_data: inner_data.clone(),
            shared_value,
        };

        (inner_data, wrapper, rcv_ref)
    }
}

impl<OuterState, Sch, Values, Rcv> Receiver for InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_error(self, error: Error) {
        if self
            .shared_value
            .borrow_mut()
            .set(Signal::Error(error))
            .is_err()
        {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
    }

    fn set_done(self) {
        if self.shared_value.borrow_mut().set(Signal::Done).is_err() {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
    }
}

impl<OuterState, Sch, Values, Rcv> Receiver for InnerScopeSendReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_error(self, error: Error) {
        let mut opt_shared_value = self.shared_value.lock().unwrap();
        if opt_shared_value.is_some() {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
        let _ = opt_shared_value.insert(Signal::Error(error));
    }

    fn set_done(self) {
        let mut opt_shared_value = self.shared_value.lock().unwrap();
        if opt_shared_value.is_some() {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
        let _ = opt_shared_value.insert(Signal::Done);
    }
}

impl<OuterState, Sch, Values, Rcv> ReceiverOf<Sch, Values>
    for InnerScopeReceiver<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_value(self, sch: Sch, values: Values) {
        if self
            .shared_value
            .borrow_mut()
            .set(Signal::Value(sch, values))
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
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    fn set_value(self, sch: Sch, values: Values) {
        let mut opt_shared_value = self.shared_value.lock().unwrap();
        if opt_shared_value.is_some() {
            self.inner_data.mark_panicked();
            panic!("signal double assigned");
        }
        let _ = opt_shared_value.insert(Signal::Value(sch, values));
    }
}

fn forward<Sch, Values, Rcv>(rcv: Rcv, argument: Option<Signal<Sch, Values>>)
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    if let Some(argument) = argument {
        match argument {
            Signal::Value(sch, values) => rcv.set_value(sch, values),
            Signal::Error(error) => rcv.set_error(error),
            Signal::Done => rcv.set_done(),
        }
    }
}

type SharedValueNoSend<Sch, Values> = Rc<RefCell<OnceCell<Signal<Sch, Values>>>>;
type SharedValueSend<Sch, Values> = Arc<Mutex<Option<Signal<Sch, Values>>>>;

struct ScopeDataNoSend<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    data: Rc<RefCell<ScopeDataNoSend<OuterState, Sch, Values, Rcv>>>,
}

pub struct ScopeDataSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    data: Arc<ScopeDataSend<OuterState, Sch, Values, Rcv>>,
}

impl<OuterState, Sch, Values, Rcv> ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    type NewScopeType<NestedSch, NestedValues, NestedRcv> = ScopeDataNoSendPtr<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler<LocalScheduler=NestedSch>, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;
    type NewReceiver<NestedSch, NestedValues, NestedRcv> = InnerScopeReceiver<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler<LocalScheduler=NestedSch>, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;

    fn new_scope<NestedSch, NestedValues, NestedRcv>(
        &self,
        rcv: NestedRcv,
    ) -> (
        Self::NewScopeType<NestedSch, NestedValues, NestedRcv>,
        Self::NewReceiver<NestedSch, NestedValues, NestedRcv>,
        ScopedRefMut<NestedRcv, Self::NewScopeType<NestedSch, NestedValues, NestedRcv>>,
    )
    where
        NestedSch: Scheduler<LocalScheduler = NestedSch>,
        NestedValues: Tuple,
        NestedRcv: ReceiverOf<NestedSch, NestedValues>,
    {
        Self::NewReceiver::<NestedSch, NestedValues, NestedRcv>::new(self, rcv)
    }

    fn mark_panicked(&self) {
        self.data.borrow_mut().mark_panicked();
    }
}

impl<OuterState, Sch, Values, Rcv> ScopeData for ScopeDataSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
    Values: Tuple,
    Rcv: ReceiverOf<Sch, Values>,
{
    type NewScopeType<NestedSch, NestedValues, NestedRcv> = ScopeDataSendPtr<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler<LocalScheduler=NestedSch>, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;
    type NewReceiver<NestedSch, NestedValues, NestedRcv> = InnerScopeSendReceiver<Self, NestedSch, NestedValues, NestedRcv>
    where NestedSch:Scheduler<LocalScheduler=NestedSch>, NestedValues:Tuple, NestedRcv: ReceiverOf<NestedSch, NestedValues>;

    fn new_scope<NestedSch, NestedValues, NestedRcv>(
        &self,
        rcv: NestedRcv,
    ) -> (
        Self::NewScopeType<NestedSch, NestedValues, NestedRcv>,
        Self::NewReceiver<NestedSch, NestedValues, NestedRcv>,
        ScopedRefMut<NestedRcv, Self::NewScopeType<NestedSch, NestedValues, NestedRcv>>,
    )
    where
        NestedSch: Scheduler<LocalScheduler = NestedSch>,
        NestedValues: Tuple,
        NestedRcv: ReceiverOf<NestedSch, NestedValues>,
    {
        Self::NewReceiver::<NestedSch, NestedValues, NestedRcv>::new(self, rcv)
    }

    fn mark_panicked(&self) {
        self.data.mark_panicked();
    }
}

impl<OuterState, Sch, Values, Rcv> fmt::Debug for ScopeDataNoSendPtr<OuterState, Sch, Values, Rcv>
where
    OuterState: ScopeData,
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
                Signal::Value(..) => "holding value signal",
                Signal::Error(..) => "holding error signal",
                Signal::Done => "holding done signal",
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
    Sch: Scheduler<LocalScheduler = Sch>,
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
                Signal::Value(..) => "holding value signal",
                Signal::Error(..) => "holding error signal",
                Signal::Done => "holding done signal",
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
