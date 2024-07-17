//! [LetValue] needs its own flavour of functors, that we can't express in the normal functors.

use crate::errors::Result;
use crate::refs::ScopedRefMut;
use std::fmt::Debug;
use std::marker::PhantomData;

/// A bi-functor can be invoked with two arguments.
///
/// The second argument represents a tuple of arguments.
pub trait BiFunctor<'a, FirstArg, Args, State>
where
    State: 'static + Clone + Debug,
{
    /// The result type of the functor.
    type Output;

    /// Invoke this functor.
    fn tuple_invoke(self, first_arg: FirstArg, args: ScopedRefMut<Args, State>) -> Self::Output;
}

/// BiFunctor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct BiClosure<'a, FnType, Out, FirstArg, Args, State>
where
    FnType: 'a + FnOnce(FirstArg, ScopedRefMut<Args, State>) -> Out,
    State: 'static + Clone + Debug,
{
    phantom: PhantomData<&'a fn(FirstArg, Args, State) -> Out>,
    fn_impl: FnType,
}

impl<'a, FnType, Out, FirstArg, Args, State> BiClosure<'a, FnType, Out, FirstArg, Args, State>
where
    FnType: 'a + FnOnce(FirstArg, ScopedRefMut<Args, State>) -> Out,
    State: 'static + Clone + Debug,
{
    /// Create a new [BiClosure] from the given function.
    pub fn new(fn_impl: FnType) -> Self {
        BiClosure {
            phantom: PhantomData,
            fn_impl,
        }
    }
}

impl<'a, FnType, Out, FirstArg, Args, State> BiFunctor<'a, FirstArg, Args, State>
    for BiClosure<'a, FnType, Out, FirstArg, Args, State>
where
    FnType: 'a + FnOnce(FirstArg, ScopedRefMut<Args, State>) -> Out,
    State: 'static + Clone + Debug,
{
    type Output = Out;

    fn tuple_invoke(self, first_arg: FirstArg, args: ScopedRefMut<Args, State>) -> Self::Output {
        (self.fn_impl)(first_arg, args)
    }
}

/// Wrapper for bi-functors that don't return a [Result].
/// This wrapper wraps the functor into something that will return a [Result].
///
/// We need to use an actual struct, because we need to declare types.
/// With closures, we cannot capture the closure type, and thus not create a specialization.
pub struct NoErrBiFunctor<'a, FunctorType, Out, FirstArg, ArgTuple, State>
where
    FunctorType: BiFunctor<'a, FirstArg, ArgTuple, State, Output = Out>,
    State: 'static + Clone + Debug,
{
    functor: FunctorType,
    phantom: PhantomData<&'a fn(FirstArg, ArgTuple, State) -> Out>,
}

impl<'a, FunctorType, Out, FirstArg, ArgTuple, State>
    NoErrBiFunctor<'a, FunctorType, Out, FirstArg, ArgTuple, State>
where
    FunctorType: BiFunctor<'a, FirstArg, ArgTuple, State, Output = Out>,
    State: 'static + Clone + Debug,
{
    /// Create a new [NoErrBiFunctor] from the given function.
    pub fn new(functor: FunctorType) -> Self {
        NoErrBiFunctor {
            functor,
            phantom: PhantomData,
        }
    }
}

impl<'a, FunctorType, Out, FirstArg, ArgTuple, State> BiFunctor<'a, FirstArg, ArgTuple, State>
    for NoErrBiFunctor<'a, FunctorType, Out, FirstArg, ArgTuple, State>
where
    FunctorType: BiFunctor<'a, FirstArg, ArgTuple, State, Output = Out>,
    State: 'static + Clone + Debug,
{
    type Output = Result<Out>;

    fn tuple_invoke(
        self,
        first_arg: FirstArg,
        args: ScopedRefMut<ArgTuple, State>,
    ) -> Self::Output {
        Ok(self.functor.tuple_invoke(first_arg, args))
    }
}
