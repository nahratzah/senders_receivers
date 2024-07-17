//! Reference logic for senders/receivers.
//!
//! In order to make [LetValue](crate::let_value::LetValue) work, we need to pass arguments by reference.
//! And in order to make those references work, we need to have either higher-order references, or use a scoped reference.
//! We use the latter, because I couldn't make the former work.

use crate::tuple::tuple_impls;
use crate::tuple::DistributeRefTuple;
use std::fmt;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;

/// Reference which is bound to a scope.
/// It ensures the reference remains valid, by keeping the scope live.
pub struct ScopedRef<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    actual: *const T, // never null
    state: State,
}

/// Reference which is bound to a scope.
/// It ensures the reference remains valid, by keeping the scope live.
pub struct ScopedRefMut<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    actual: *mut T, // never null
    state: State,
}

impl<T, State> ScopedRef<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    /// Create a new [ScopedRef].
    ///
    /// # Safety
    ///
    /// The referent must have a lifetime equal to the State,
    /// and its lifetime must be guaranteed by the State.
    pub unsafe fn new(r: &'_ T, state: State) -> Self {
        Self { actual: r, state }
    }

    /// Clone the reference.
    pub fn clone(r: Self) -> Self {
        unsafe { Self::new(&*r.actual, r.state.clone()) }
    }

    /// Transform the reference into a dependant reference.
    pub fn map<F, U>(r: Self, f: F) -> ScopedRef<U, State>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        unsafe { ScopedRef::new(f(&*r.actual), r.state) }
    }

    /// Convert the reference into a dependent reference.
    /// If the callback returns [None], the original reference will be returned in the [Err] result.
    pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRef<U, State>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        match f(unsafe { &*r.actual }) {
            Some(actual) => unsafe { Ok(ScopedRef::new(actual, r.state)) },
            None => Err(r),
        }
    }

    /// Split the reference in two references.
    pub fn map_split<F, U, V>(r: Self, f: F) -> (ScopedRef<U, State>, ScopedRef<V, State>)
    where
        F: FnOnce(&T) -> (&U, &V),
        U: ?Sized,
        V: ?Sized,
    {
        let (u, v) = f(unsafe { &*r.actual });
        let u = unsafe { ScopedRef::new(u, r.state.clone()) };
        let v = unsafe { ScopedRef::new(v, r.state) };
        (u, v)
    }
}

impl<T, State> ScopedRefMut<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    /// Create a new [ScopedRef].
    ///
    /// # Safety
    ///
    /// The referent must have a lifetime equal to the State,
    /// and its lifetime must be guaranteed by the State.
    pub unsafe fn new(r: &'_ mut T, state: State) -> Self {
        Self { actual: r, state }
    }

    /// Transform the reference into a dependant reference.
    pub fn map<F, U>(r: Self, f: F) -> ScopedRefMut<U, State>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        unsafe { ScopedRefMut::new(f(&mut *r.actual), r.state) }
    }

    /// Transform the reference into a dependant, non-mutable reference.
    pub fn map_no_mut<F, U>(r: Self, f: F) -> ScopedRef<U, State>
    where
        F: FnOnce(&mut T) -> &U,
        U: ?Sized,
    {
        unsafe { ScopedRef::new(f(&mut *r.actual), r.state) }
    }

    /// Convert the reference into a dependent reference.
    /// If the callback returns [None], the original reference will be returned in the [Err] result.
    pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRefMut<U, State>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // Current borrow-checker can't do this without unsafe.
        let raw_result = unsafe {
            match f(&mut *r.actual) {
                Some(y) => Ok(y),
                None => Err(&mut *r.actual),
            }
        };

        match raw_result {
            Ok(new_ref) => Ok(unsafe { ScopedRefMut::new(new_ref, r.state) }),
            Err(new_ref) => Err(unsafe { ScopedRefMut::new(new_ref, r.state) }),
        }
    }

    /// Split the reference in two references.
    pub fn map_split<F, U, V>(r: Self, f: F) -> (ScopedRefMut<U, State>, ScopedRefMut<V, State>)
    where
        F: FnOnce(&mut T) -> (&mut U, &mut V),
        U: ?Sized,
        V: ?Sized,
    {
        let (u, v) = f(unsafe { &mut *r.actual });
        let u = unsafe { ScopedRefMut::new(u, r.state.clone()) };
        let v = unsafe { ScopedRefMut::new(v, r.state) };
        (u, v)
    }
}

impl<T, State> Deref for ScopedRef<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.actual }
    }
}

impl<T, State> Deref for ScopedRefMut<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.actual }
    }
}

impl<T, State> DerefMut for ScopedRefMut<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.actual }
    }
}

impl<T, State> fmt::Display for ScopedRef<T, State>
where
    T: ?Sized + fmt::Display,
    State: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (unsafe { &*self.actual }).fmt(f)
    }
}

impl<T, State> fmt::Display for ScopedRefMut<T, State>
where
    T: ?Sized + fmt::Display,
    State: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (unsafe { &*self.actual }).fmt(f)
    }
}

impl<T, State> fmt::Debug for ScopedRef<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedRef")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<T, State> fmt::Debug for ScopedRefMut<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedRefMut")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

macro_rules! make_distribute_scoped_ref {
    () => {
        impl<State> DistributeRefTuple for ScopedRef<(), State>
        where
            State: Clone + fmt::Debug,
        {
            type Output = ();

            fn distribute(_: Self) -> Self::Output {

            }
        }

        impl<State> DistributeRefTuple for ScopedRefMut<(), State>
        where
            State: Clone + fmt::Debug,
        {
            type Output = ();

            fn distribute(_: Self) -> Self::Output {

            }
        }
    };
    ($v:ident : $T:ident) => {
        impl<$T, State> DistributeRefTuple for ScopedRef<($T,), State>
        where
            $T: ?Sized,
            State: Clone + fmt::Debug,
        {
            type Output = (ScopedRef<$T, State>,);

            fn distribute(x: Self) -> Self::Output {
                (ScopedRef::map(x,|v: &($T,)| &v.0),)
            }
        }

        impl<$T, State> DistributeRefTuple for ScopedRefMut<($T,), State>
        where
            $T: ?Sized,
            State: Clone + fmt::Debug,
        {
            type Output = (ScopedRefMut<$T, State>,);

            fn distribute(x: Self) -> Self::Output {
                (ScopedRefMut::map(x, |v: &mut ($T,)| &mut v.0),)
            }
        }
    };
    ($($v:ident : $T:ident),+) => {
        impl<$($T),+, State> DistributeRefTuple for ScopedRef<($($T),+), State>
        where
            for<'a> &'a ($($T),+): DistributeRefTuple<Output=($(&'a $T),+)>,
            State: Clone + fmt::Debug,
        {
            type Output = ($(ScopedRef<$T, State>),+);

            fn distribute(x: Self) -> Self::Output {
                let ($($v),+) = DistributeRefTuple::distribute(unsafe{&*x.actual});
                ($(unsafe{ScopedRef::new($v, x.state.clone())}),+)
            }
        }

        impl<$($T),+, State> DistributeRefTuple for ScopedRefMut<($($T),+), State>
        where
            for<'a> &'a mut ($($T),+): DistributeRefTuple<Output=($(&'a mut $T),+)>,
            State: Clone + fmt::Debug,
        {
            type Output = ($(ScopedRefMut<$T, State>),+);

            fn distribute(x: Self) -> Self::Output {
                let ($($v),+) = DistributeRefTuple::distribute(unsafe{&mut *x.actual});
                ($(unsafe{ScopedRefMut::new($v, x.state.clone())}),+)
            }
        }
    };
}

tuple_impls!(make_distribute_scoped_ref);

type DebugFn = dyn Fn(&mut fmt::Formatter<'_>) -> fmt::Result;
type LifetimedDebugFn<'a> = dyn 'a + Fn(&mut fmt::Formatter<'_>) -> fmt::Result;
type DebugSendFn = dyn Send + Sync + Fn(&mut fmt::Formatter<'_>) -> fmt::Result;
type LifetimedDebugSendFn<'a> = dyn 'a + Send + Sync + Fn(&mut fmt::Formatter<'_>) -> fmt::Result;

/// State type used by [ScopedRef] and [ScopedRefMut] for sendable state.
#[derive(Clone)]
pub struct SendState {
    data: Arc<DebugSendFn>,
}

/// State type used by [ScopedRef] and [ScopedRefMut] for unsendable state.
#[derive(Clone)]
pub struct NoSendState {
    data: Rc<DebugFn>,
}

impl fmt::Debug for SendState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.data)(f)
    }
}

impl fmt::Debug for NoSendState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.data)(f)
    }
}

impl SendState {
    fn new<State>(state: State) -> Self
    where
        State: Sync + Send + fmt::Debug,
    {
        let data: Option<Arc<LifetimedDebugSendFn>> =
            Some(Arc::new(move |f: &mut fmt::Formatter<'_>| state.fmt(f)));
        let data = unsafe {
            mem::transmute::<Option<Arc<LifetimedDebugSendFn>>, Option<Arc<DebugSendFn>>>(data)
                .take()
                .unwrap()
        };

        Self { data }
    }
}

impl NoSendState {
    fn new<State>(state: State) -> Self
    where
        State: fmt::Debug,
    {
        let data: Option<Rc<LifetimedDebugFn>> =
            Some(Rc::new(move |f: &mut fmt::Formatter<'_>| state.fmt(f)));
        let data = unsafe {
            mem::transmute::<Option<Rc<LifetimedDebugFn>>, Option<Rc<DebugFn>>>(data)
                .take()
                .unwrap()
        };

        Self { data }
    }
}

/// We wanted to implement [From] for ScopedRefMut --> ScopedRefMut conversions.
/// Sadly, this is impossible right now, because From has an implementation for conversions between its own type.
/// Which we can't exclude.
/// So we use a separate implementation instead.
pub trait SRFrom<T> {
    /// Construct from a value.
    fn sr_from(r: T) -> Self;
}

/// Similarly to [SRFrom], we can't implement the [Into] state directly.
/// So we have to implement a mirror of it.
pub trait SRInto<T> {
    /// Turn this into the indicated type.
    /// The actual type is inferred.
    fn sr_into(self) -> T;
}

impl<T, State> SRFrom<ScopedRefMut<T, State>> for ScopedRefMut<T, SendState>
where
    T: ?Sized,
    State: Sync + Send + Clone + fmt::Debug,
{
    fn sr_from(r: ScopedRefMut<T, State>) -> Self {
        unsafe { Self::new(&mut *r.actual, SendState::new(r.state)) }
    }
}

impl<T, State> SRFrom<ScopedRefMut<T, State>> for ScopedRefMut<T, NoSendState>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    fn sr_from(r: ScopedRefMut<T, State>) -> Self {
        unsafe { Self::new(&mut *r.actual, NoSendState::new(r.state)) }
    }
}

impl<T, State> SRFrom<ScopedRef<T, State>> for ScopedRef<T, SendState>
where
    T: ?Sized,
    State: Sync + Send + Clone + fmt::Debug,
{
    fn sr_from(r: ScopedRef<T, State>) -> Self {
        unsafe { Self::new(&*r.actual, SendState::new(r.state)) }
    }
}

impl<T, State> SRFrom<ScopedRef<T, State>> for ScopedRef<T, NoSendState>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    fn sr_from(r: ScopedRef<T, State>) -> Self {
        unsafe { Self::new(&*r.actual, NoSendState::new(r.state)) }
    }
}

impl<T, State> SRInto<ScopedRefMut<T, SendState>> for ScopedRefMut<T, State>
where
    T: ?Sized,
    ScopedRefMut<T, SendState>: SRFrom<Self>,
    State: Clone + fmt::Debug,
{
    fn sr_into(self) -> ScopedRefMut<T, SendState> {
        ScopedRefMut::sr_from(self)
    }
}

impl<T, State> SRInto<ScopedRef<T, SendState>> for ScopedRef<T, State>
where
    T: ?Sized,
    ScopedRef<T, SendState>: SRFrom<Self>,
    State: Clone + fmt::Debug,
{
    fn sr_into(self) -> ScopedRef<T, SendState> {
        ScopedRef::sr_from(self)
    }
}

impl<T, State> SRInto<ScopedRefMut<T, NoSendState>> for ScopedRefMut<T, State>
where
    T: ?Sized,
    ScopedRefMut<T, NoSendState>: SRFrom<Self>,
    State: Clone + fmt::Debug,
{
    fn sr_into(self) -> ScopedRefMut<T, NoSendState> {
        ScopedRefMut::sr_from(self)
    }
}

impl<T, State> SRInto<ScopedRef<T, NoSendState>> for ScopedRef<T, State>
where
    T: ?Sized,
    ScopedRef<T, NoSendState>: SRFrom<Self>,
    State: Clone + fmt::Debug,
{
    fn sr_into(self) -> ScopedRef<T, NoSendState> {
        ScopedRef::sr_from(self)
    }
}

impl<T, State> From<ScopedRefMut<T, State>> for ScopedRef<T, State>
where
    T: ?Sized,
    State: Clone + fmt::Debug,
{
    fn from(r: ScopedRefMut<T, State>) -> Self {
        unsafe { Self::new(&*r.actual, r.state) }
    }
}
