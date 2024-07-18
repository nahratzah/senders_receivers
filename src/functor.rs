//! The `functors` mod holds functions.
//!
//! The reason we need those, is because we need to have composible functions.
//! The [Functor] and [BiFunctor] traits provide something we can invoke.
//!
//! Note: we might erase this module in the future, if implementing [FnOnce] becomes doable.

use crate::errors::Result;
use std::marker::PhantomData;

/// Functor that takes no arguments.
pub trait NoArgFunctor<'a> {
    /// Type returned by the functor invocation.
    type Output;

    /// Invoke this functor, producing a new [Self::Output].
    fn tuple_invoke(self) -> Self::Output;
}

/// A functor can be invoked.
///
/// Args represents a tuple of arguments.
pub trait Functor<'a, Args> {
    /// The result type of the functor.
    type Output;

    /// Invoke this functor.
    ///
    /// The function name is `tuple_invoke`, because:
    /// - we invoke it with tuple only;
    /// - I wanted a name that wouldn't clash, if in the future variadic traits became a thing.
    fn tuple_invoke(self, args: Args) -> Self::Output;
}

/// A bi-functor can be invoked with two arguments.
///
/// The second argument represents a tuple of arguments.
pub trait BiFunctor<'a, FirstArg, Args> {
    /// The result type of the functor.
    type Output;

    /// Invoke this functor.
    fn tuple_invoke(self, first_arg: FirstArg, args: Args) -> Self::Output;
}

/// Functor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct NoArgClosure<'a, FnType, Out>
where
    FnType: 'a + FnOnce() -> Out,
{
    phantom: PhantomData<&'a fn() -> Out>,
    fn_impl: FnType,
}

impl<'a, FnType, Out> NoArgClosure<'a, FnType, Out>
where
    FnType: 'a + FnOnce() -> Out,
{
    /// Create a new [NoArgClosure] from the given function.
    pub fn new(fn_impl: FnType) -> Self {
        Self {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

impl<'a, FnType, Out> NoArgFunctor<'a> for NoArgClosure<'a, FnType, Out>
where
    FnType: 'a + FnOnce() -> Out,
{
    type Output = Out;

    fn tuple_invoke(self) -> Self::Output {
        (self.fn_impl)()
    }
}

/// Functor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct Closure<'a, FnType, Out, Args>
where
    FnType: 'a + FnOnce(Args) -> Out,
{
    phantom: PhantomData<&'a fn(Args) -> Out>,
    fn_impl: FnType,
}

impl<'a, FnType, Out, Args> Closure<'a, FnType, Out, Args>
where
    FnType: 'a + FnOnce(Args) -> Out,
{
    /// Create a new [Closure] from the given function.
    pub fn new(fn_impl: FnType) -> Self {
        Closure {
            phantom: PhantomData,
            fn_impl,
        }
    }
}

impl<'a, FnType, Out, Args> Functor<'a, Args> for Closure<'a, FnType, Out, Args>
where
    FnType: 'a + FnOnce(Args) -> Out,
{
    type Output = Out;

    fn tuple_invoke(self, args: Args) -> Self::Output {
        let fn_impl = self.fn_impl;
        fn_impl(args)
    }
}

/// BiFunctor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct BiClosure<'a, FnType, Out, FirstArg, Args>
where
    FnType: 'a + FnOnce(FirstArg, Args) -> Out,
{
    phantom: PhantomData<&'a fn(FirstArg, Args) -> Out>,
    fn_impl: FnType,
}

impl<'a, FnType, Out, FirstArg, Args> BiClosure<'a, FnType, Out, FirstArg, Args>
where
    FnType: 'a + FnOnce(FirstArg, Args) -> Out,
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
    FnType: 'a + FnOnce(FirstArg, Args) -> Out,
{
    type Output = Out;

    fn tuple_invoke(self, first_arg: FirstArg, args: Args) -> Self::Output {
        let fn_impl = self.fn_impl;
        fn_impl(first_arg, args)
    }
}

/// Wrapper for functors that don't return a [Result].
/// This wrapper wraps the functor into something that will return a [Result].
pub struct NoErrNoArgFunctor<'a, FunctorType, Out>
where
    FunctorType: NoArgFunctor<'a, Output = Out>,
{
    functor: FunctorType,
    phantom: PhantomData<&'a fn() -> Out>,
}

impl<'a, FunctorType, Out> NoErrNoArgFunctor<'a, FunctorType, Out>
where
    FunctorType: NoArgFunctor<'a, Output = Out>,
{
    /// Create a new [NoErrNoArgFunctor] from the given function.
    pub fn new(functor: FunctorType) -> Self {
        Self {
            functor,
            phantom: PhantomData,
        }
    }
}

impl<'a, FunctorType, Out> NoArgFunctor<'a> for NoErrNoArgFunctor<'a, FunctorType, Out>
where
    FunctorType: NoArgFunctor<'a, Output = Out>,
{
    type Output = Result<Out>;

    fn tuple_invoke(self) -> Self::Output {
        Ok(self.functor.tuple_invoke())
    }
}

/// Wrapper for functors that don't return a [Result].
/// This wrapper wraps the functor into something that will return a [Result].
///
/// We need to use an actual struct, because we need to declare types.
/// With closures, we cannot capture the closure type, and thus not create a specialization.
pub struct NoErrFunctor<'a, FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<'a, ArgTuple, Output = Out>,
{
    functor: FunctorType,
    phantom: PhantomData<&'a fn(ArgTuple) -> Out>,
}

impl<'a, FunctorType, Out, ArgTuple> NoErrFunctor<'a, FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<'a, ArgTuple, Output = Out>,
{
    /// Create a new [NoErrFunctor] from the given function.
    pub fn new(functor: FunctorType) -> Self {
        NoErrFunctor {
            functor,
            phantom: PhantomData,
        }
    }
}

impl<'a, FunctorType, Out, ArgTuple> Functor<'a, ArgTuple>
    for NoErrFunctor<'a, FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<'a, ArgTuple, Output = Out>,
{
    type Output = Result<Out>;

    fn tuple_invoke(self, args: ArgTuple) -> Self::Output {
        Ok(self.functor.tuple_invoke(args))
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
    phantom: PhantomData<&'a fn(FirstArg, ArgTuple) -> Out>,
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

    fn tuple_invoke(self, first_arg: FirstArg, args: ArgTuple) -> Self::Output {
        Ok(self.functor.tuple_invoke(first_arg, args))
    }
}
