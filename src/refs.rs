use crate::scope::Scope;
use std::fmt;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct ScopedRef<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: for<'env> Scope<'scope, 'env>,
{
    actual: &'scope T,
    state: State,
}

#[derive(Debug)]
pub struct ScopedRefMut<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: for<'env> Scope<'scope, 'env>,
{
    actual: &'scope mut T,
    state: State,
}

impl<'a, T, State> ScopedRef<'a, T, State>
where
    T: 'a + ?Sized,
    State: for<'env> Scope<'a, 'env>,
{
    pub fn new(r: &'a T, state: State) -> Self {
        Self {
            actual: r,
            state: state,
        }
    }

    pub fn clone(r: Self) -> Self {
        Self {
            actual: r.actual,
            state: r.state.clone(),
        }
    }

    pub fn map<F, U>(r: Self, f: F) -> ScopedRef<'a, U, State>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        ScopedRef {
            actual: f(r.actual),
            state: r.state,
        }
    }

    pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRef<'a, U, State>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        match f(r.actual) {
            Some(actual) => Ok(ScopedRef {
                actual,
                state: r.state,
            }),
            None => Err(r),
        }
    }

    pub fn map_split<F, U, V>(r: Self, f: F) -> (ScopedRef<'a, U, State>, ScopedRef<'a, V, State>)
    where
        F: FnOnce(&T) -> (&U, &V),
        U: ?Sized,
        V: ?Sized,
    {
        let (u, v) = f(r.actual);
        let u = ScopedRef {
            actual: u,
            state: r.state.clone(),
        };
        let v = ScopedRef {
            actual: v,
            state: r.state,
        };
        (u, v)
    }
}

impl<'a, T, State> ScopedRefMut<'a, T, State>
where
    T: 'a + ?Sized,
    State: for<'env> Scope<'a, 'env>,
{
    pub fn map<F, U>(r: Self, f: F) -> ScopedRefMut<'a, U, State>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        ScopedRefMut {
            actual: f(r.actual),
            state: r.state,
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
    //pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRefMut<'a, U, State>, Self>
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

    pub fn map_split<F, U, V>(
        r: Self,
        f: F,
    ) -> (ScopedRefMut<'a, U, State>, ScopedRefMut<'a, V, State>)
    where
        F: FnOnce(&mut T) -> (&mut U, &mut V),
        U: ?Sized,
        V: ?Sized,
    {
        let (u, v) = f(r.actual);
        let u = ScopedRefMut {
            actual: u,
            state: r.state.clone(),
        };
        let v = ScopedRefMut {
            actual: v,
            state: r.state,
        };
        (u, v)
    }
}

impl<'a, T, State> Deref for ScopedRef<'a, T, State>
where
    T: 'a + ?Sized,
    State: for<'env> Scope<'a, 'env>,
{
    type Target = T;

    fn deref(&self) -> &T {
        self.actual
    }
}

impl<'a, T, State> Deref for ScopedRefMut<'a, T, State>
where
    T: 'a + ?Sized,
    State: for<'env> Scope<'a, 'env>,
{
    type Target = T;

    fn deref(&self) -> &T {
        self.actual
    }
}

impl<'a, T, State> DerefMut for ScopedRefMut<'a, T, State>
where
    T: 'a + ?Sized,
    State: for<'env> Scope<'a, 'env>,
{
    fn deref_mut(&mut self) -> &mut T {
        self.actual
    }
}

impl<'a, T, State> fmt::Display for ScopedRefMut<'a, T, State>
where
    T: 'a + ?Sized + fmt::Display,
    State: for<'env> Scope<'a, 'env>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.actual.fmt(f)
    }
}
