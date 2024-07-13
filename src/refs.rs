use crate::tuple::tuple_impls;
use crate::tuple::DistributeRefTuple;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// Reference which is bound to a scope.
/// It ensures the reference remains valid, by keeping the scope live.
pub struct ScopedRef<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    actual: &'scope T,
    state: State,
}

/// Reference which is bound to a scope.
/// It ensures the reference remains valid, by keeping the scope live.
pub struct ScopedRefMut<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    actual: &'scope mut T,
    state: State,
}

impl<'scope, T, State> ScopedRef<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    /// Create a new [ScopedRef].
    /// The referent must have a lifetime equal to the State,
    /// and its lifetime must be guaranteed by the State.
    pub fn new(r: &'scope T, state: State) -> Self {
        Self {
            actual: r,
            state: state,
        }
    }

    /// Clone the reference.
    pub fn clone(r: Self) -> Self {
        Self {
            actual: r.actual,
            state: r.state.clone(),
        }
    }

    /// Transform the reference into a dependant reference.
    pub fn map<F, U>(r: Self, f: F) -> ScopedRef<'scope, U, State>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        ScopedRef {
            actual: f(r.actual),
            state: r.state,
        }
    }

    /// Convert the reference into a dependent reference.
    /// If the callback returns [None], the original reference will be returned in the [Err] result.
    pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRef<'scope, U, State>, Self>
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

    /// Split the reference in two references.
    pub fn map_split<F, U, V>(
        r: Self,
        f: F,
    ) -> (ScopedRef<'scope, U, State>, ScopedRef<'scope, V, State>)
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

impl<'scope, T, State> ScopedRefMut<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    /// Create a new [ScopedRef].
    /// The referent must have a lifetime equal to the State,
    /// and its lifetime must be guaranteed by the State.
    pub fn new(r: &'scope mut T, state: State) -> Self {
        Self {
            actual: r,
            state: state,
        }
    }

    /// Transform the reference into a dependant reference.
    pub fn map<F, U>(r: Self, f: F) -> ScopedRefMut<'scope, U, State>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        ScopedRefMut {
            actual: f(r.actual),
            state: r.state,
        }
    }

    /// Convert the reference into a dependent reference.
    /// If the callback returns [None], the original reference will be returned in the [Err] result.
    pub fn filter_map<F, U>(r: Self, f: F) -> Result<ScopedRefMut<'scope, U, State>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // Current borrow-checker can't do this without unsafe.
        let raw_result = unsafe {
            let x_ptr: *mut T = r.actual;
            match f(&mut *x_ptr) {
                Some(y) => Ok(y),
                None => Err(&mut *x_ptr),
            }
        };

        match raw_result {
            Ok(new_ref) => Ok(ScopedRefMut {
                actual: new_ref,
                state: r.state,
            }),
            Err(new_ref) => Err(ScopedRefMut {
                actual: new_ref,
                state: r.state,
            }),
        }
    }

    /// Split the reference in two references.
    pub fn map_split<F, U, V>(
        r: Self,
        f: F,
    ) -> (
        ScopedRefMut<'scope, U, State>,
        ScopedRefMut<'scope, V, State>,
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
        };
        let v = ScopedRefMut {
            actual: v,
            state: r.state,
        };
        (u, v)
    }
}

impl<'scope, T, State> Deref for ScopedRef<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    type Target = T;

    fn deref(&self) -> &T {
        self.actual
    }
}

impl<'scope, T, State> Deref for ScopedRefMut<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    type Target = T;

    fn deref(&self) -> &T {
        self.actual
    }
}

impl<'scope, T, State> DerefMut for ScopedRefMut<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    fn deref_mut(&mut self) -> &mut T {
        self.actual
    }
}

impl<'scope, T, State> fmt::Display for ScopedRefMut<'scope, T, State>
where
    T: 'scope + ?Sized + fmt::Display,
    State: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.actual.fmt(f)
    }
}

impl<'scope, T, State> fmt::Debug for ScopedRef<'scope, T, State>
where
    T: 'scope + ?Sized,
    State: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedRef")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<'scope, T, State> fmt::Debug for ScopedRefMut<'scope, T, State>
where
    T: 'scope + ?Sized,
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
        impl<'scope, State> DistributeRefTuple for ScopedRef<'scope, (), State>
        where
            State: Clone + fmt::Debug,
        {
            type Output = ();

            fn distribute(_: Self) -> Self::Output {
                ()
            }
        }

        impl<'scope, State> DistributeRefTuple for ScopedRefMut<'scope, (), State>
        where
            State: Clone + fmt::Debug,
        {
            type Output = ();

            fn distribute(_: Self) -> Self::Output {
                ()
            }
        }
    };
    ($v:ident : $T:ident) => {
        impl<'scope, $T, State> DistributeRefTuple for ScopedRef<'scope, ($T,), State>
        where
            $T: 'scope + ?Sized,
            State: Clone + fmt::Debug,
        {
            type Output = (ScopedRef<'scope, $T, State>,);

            fn distribute(x: Self) -> Self::Output {
                (ScopedRef::map(x,|v: &($T,)| &v.0),)
            }
        }

        impl<'scope, $T, State> DistributeRefTuple for ScopedRefMut<'scope, ($T,), State>
        where
            $T: 'scope + ?Sized,
            State: Clone + fmt::Debug,
        {
            type Output = (ScopedRefMut<'scope, $T, State>,);

            fn distribute(x: Self) -> Self::Output {
                (ScopedRefMut::map(x, |v: &mut ($T,)| &mut v.0),)
            }
        }
    };
    ($($v:ident : $T:ident),+) => {
        impl<'scope, $($T),+, State> DistributeRefTuple for ScopedRef<'scope, ($($T),+), State>
        where
            $($T: 'scope,)+
            for<'a> &'a ($($T),+): DistributeRefTuple<Output=($(&'a $T),+)>,
            State: Clone + fmt::Debug,
        {
            type Output = ($(ScopedRef<'scope, $T, State>),+);

            fn distribute(x: Self) -> Self::Output {
                let ($($v),+) = DistributeRefTuple::distribute(x.actual);
                (
                    $(ScopedRef{
                        actual: $v,
                        state: x.state.clone(),
                    }),+
                )
            }
        }

        impl<'scope, $($T),+, State> DistributeRefTuple for ScopedRefMut<'scope, ($($T),+), State>
        where
            $($T: 'scope,)+
            for<'a> &'a mut ($($T),+): DistributeRefTuple<Output=($(&'a mut $T),+)>,
            State: Clone + fmt::Debug,
        {
            type Output = ($(ScopedRefMut<'scope, $T, State>),+);

            fn distribute(x: Self) -> Self::Output {
                let ($($v),+) = DistributeRefTuple::distribute(x.actual);
                (
                    $(ScopedRefMut{
                        actual: $v,
                        state: x.state.clone(),
                    }),+
                )
            }
        }
    };
}

tuple_impls!(make_distribute_scoped_ref);
