//! [LetValue] needs its own flavour of functors, that we can't express in the normal functors.

use crate::errors::Result;
use std::marker::PhantomData;

/// A bi-functor can be invoked with two arguments.
///
/// The second argument represents a tuple of arguments.
pub trait BiFunctor<'a, FirstArg, Args> {
    /// The result type of the functor.
    type Output;

    /// Invoke this functor.
    fn tuple_invoke<'scope>(self, first_arg: FirstArg, args: &'scope mut Args) -> Self::Output;
}

/// BiFunctor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct BiClosure<'a, FnType, Out, FirstArg, Args>
where
    FnType: 'a + for<'scope> FnOnce(FirstArg, &'scope mut Args) -> Out,
{
    phantom: PhantomData<(&'a (), fn(FirstArg, Args) -> Out)>,
    fn_impl: FnType,
}

impl<'a, FnType, Out, FirstArg, Args> BiClosure<'a, FnType, Out, FirstArg, Args>
where
    FnType: 'a + for<'scope> FnOnce(FirstArg, &'scope mut Args) -> Out,
{
    /// Create a new [BiClosure] from the given function.
    pub fn new(fn_impl: FnType) -> Self {
        BiClosure {
            phantom: PhantomData,
            fn_impl,
        }
    }
}

impl<'a, FnType, Out, FirstArg, Args> BiFunctor<'a, FirstArg, Args>
    for BiClosure<'a, FnType, Out, FirstArg, Args>
where
    FnType: 'a + for<'scope> FnOnce(FirstArg, &'scope mut Args) -> Out,
{
    type Output = Out;

    fn tuple_invoke<'scope>(self, first_arg: FirstArg, args: &'scope mut Args) -> Self::Output {
        (self.fn_impl)(first_arg, args)
    }
}

/// Wrapper for bi-functors that don't return a [Result].
/// This wrapper wraps the functor into something that will return a [Result].
///
/// We need to use an actual struct, because we need to declare types.
/// With closures, we cannot capture the closure type, and thus not create a specialization.
pub struct NoErrBiFunctor<'a, FunctorType, Out, FirstArg, ArgTuple>
where
    FunctorType: BiFunctor<'a, FirstArg, ArgTuple, Output = Out>,
{
    functor: FunctorType,
    phantom: PhantomData<(&'a (), fn(FirstArg, ArgTuple) -> Out)>,
}

impl<'a, FunctorType, Out, FirstArg, ArgTuple>
    NoErrBiFunctor<'a, FunctorType, Out, FirstArg, ArgTuple>
where
    FunctorType: BiFunctor<'a, FirstArg, ArgTuple, Output = Out>,
{
    /// Create a new [NoErrBiFunctor] from the given function.
    pub fn new(functor: FunctorType) -> Self {
        NoErrBiFunctor {
            functor,
            phantom: PhantomData,
        }
    }
}

impl<'a, FunctorType, Out, FirstArg, ArgTuple> BiFunctor<'a, FirstArg, ArgTuple>
    for NoErrBiFunctor<'a, FunctorType, Out, FirstArg, ArgTuple>
where
    FunctorType: BiFunctor<'a, FirstArg, ArgTuple, Output = Out>,
{
    type Output = Result<Out>;

    fn tuple_invoke<'scope>(self, first_arg: FirstArg, args: &'scope mut ArgTuple) -> Self::Output {
        Ok(self.functor.tuple_invoke(first_arg, args))
    }
}
