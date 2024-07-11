use crate::scope::Scope;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// Reference which is bound to a scope.
/// It ensures the reference remains valid, by keeping the scope live.
pub struct ScopedRef<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    actual: &'scope T,
    state: State,
    phantom: PhantomData<&'env ()>,
}

/// Reference which is bound to a scope.
/// It ensures the reference remains valid, by keeping the scope live.
pub struct ScopedRefMut<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    actual: &'scope mut T,
    state: State,
    phantom: PhantomData<&'env ()>,
}

impl<'scope, 'env, T, State> ScopedRef<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    /// Create a new [ScopedRef].
    /// The referent must have a lifetime equal to the State,
    /// and its lifetime must be guaranteed by the State.
    pub fn new(r: &'scope T, state: State) -> Self {
        Self {
            actual: r,
            state: state,
            phantom: PhantomData,
        }
    }

    /// Clone the reference.
    pub fn clone(r: Self) -> Self {
        Self {
            actual: r.actual,
            state: r.state.clone(),
            phantom: PhantomData,
        }
    }

    /// Transform the reference into a dependant reference.
    pub fn map<F, U>(r: Self, f: F) -> ScopedRef<'scope, 'env, U, State>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        ScopedRef {
            actual: f(r.actual),
            state: r.state,
            phantom: PhantomData,
        }
    }

    /// Convert the reference into a dependent reference.
    /// If the callback returns [None], the original reference will be returned in the [Err] result.
    pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRef<'scope, 'env, U, State>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        match f(r.actual) {
            Some(actual) => Ok(ScopedRef {
                actual,
                state: r.state,
                phantom: PhantomData,
            }),
            None => Err(r),
        }
    }

    /// Split the reference in two references.
    pub fn map_split<F, U, V>(
        r: Self,
        f: F,
    ) -> (
        ScopedRef<'scope, 'env, U, State>,
        ScopedRef<'scope, 'env, V, State>,
    )
    where
        F: FnOnce(&T) -> (&U, &V),
        U: ?Sized,
        V: ?Sized,
    {
        let (u, v) = f(r.actual);
        let u = ScopedRef {
            actual: u,
            state: r.state.clone(),
            phantom: PhantomData,
        };
        let v = ScopedRef {
            actual: v,
            state: r.state,
            phantom: PhantomData,
        };
        (u, v)
    }
}

impl<'scope, 'env, T, State> ScopedRefMut<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    /// Create a new [ScopedRef].
    /// The referent must have a lifetime equal to the State,
    /// and its lifetime must be guaranteed by the State.
    pub fn new(r: &'scope mut T, state: State) -> Self {
        Self {
            actual: r,
            state: state,
            phantom: PhantomData,
        }
    }

    /// Transform the reference into a dependant reference.
    pub fn map<F, U>(r: Self, f: F) -> ScopedRefMut<'scope, 'env, U, State>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        ScopedRefMut {
            actual: f(r.actual),
            state: r.state,
            phantom: PhantomData,
        }
    }

    // XXX filter_map copied from std::cell::RefMut::filter_map.
    // It doesn't work. And I think that's because the code is lying,
    // and the std::cell::RefMut::filter_map isn't actually implemented as it says.
    //
    // Reasons why I believe that:
    // - the code is syntactically invalid, due to missing comma between the branches of the match statement.
    // - the compiler reject the construct, citing that the reference is borrowed twice.
    //
    // The code: https://doc.rust-lang.org/src/core/cell.rs.html#1674-1688
    //
    ///// Convert the reference into a dependent reference.
    ///// If the callback returns [None], the original reference will be returned in the [Err] result.
    //pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRefMut<'scope, 'env, U, State>, Self>
    //where F: FnOnce(&mut T) -> Option<&mut U>, U: ?Sized,
    //{
    //    match f(r.actual) {
    //        Some(new_ref) =>
    //            return Ok(ScopedRefMut{
    //                actual: new_ref,
    //                state: r.state,
    //            }),
    //        None => Err(r),
    //    }
    //}

    /// Split the reference in two references.
    pub fn map_split<F, U, V>(
        r: Self,
        f: F,
    ) -> (
        ScopedRefMut<'scope, 'env, U, State>,
        ScopedRefMut<'scope, 'env, V, State>,
    )
    where
        F: FnOnce(&mut T) -> (&mut U, &mut V),
        U: ?Sized,
        V: ?Sized,
    {
        let (u, v) = f(r.actual);
        let u = ScopedRefMut {
            actual: u,
            state: r.state.clone(),
            phantom: PhantomData,
        };
        let v = ScopedRefMut {
            actual: v,
            state: r.state,
            phantom: PhantomData,
        };
        (u, v)
    }
}

impl<'scope, 'env, T, State> Deref for ScopedRef<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    type Target = T;

    fn deref(&self) -> &T {
        self.actual
    }
}

impl<'scope, 'env, T, State> Deref for ScopedRefMut<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    type Target = T;

    fn deref(&self) -> &T {
        self.actual
    }
}

impl<'scope, 'env, T, State> DerefMut for ScopedRefMut<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    fn deref_mut(&mut self) -> &mut T {
        self.actual
    }
}

impl<'scope, 'env, T, State> fmt::Display for ScopedRefMut<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized + fmt::Display,
    State: Scope<'scope, 'env>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.actual.fmt(f)
    }
}

impl<'scope, 'env, T, State> fmt::Debug for ScopedRef<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedRef")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<'scope, 'env, T, State> fmt::Debug for ScopedRefMut<'scope, 'env, T, State>
where
    'env: 'scope,
    T: 'scope + ?Sized,
    State: Scope<'scope, 'env>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedRefMut")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}
