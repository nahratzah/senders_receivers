//! The `functors` mod holds functions.
//!
//! The reason we need those, is because we need to have composible functions.
//! The [Functor] and [BiFunctor] traits provide something we can invoke.
//!
//! Note: we might erase this module in the future, if implementing [FnOnce] becomes doable.

use crate::errors::Result;
use std::marker::PhantomData;

pub trait NoArgFunctor {
    type Output;

    fn tuple_invoke(self) -> Self::Output;
}

/// A functor can be invoked.
///
/// Args represents a tuple of arguments.
pub trait Functor<Args> {
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
pub trait BiFunctor<FirstArg, Args> {
    /// The result type of the functor.
    type Output;

    /// Invoke this functor.
    fn tuple_invoke(self, first_arg: FirstArg, args: Args) -> Self::Output;
}

/// Functor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct NoArgClosure<FnType, Out>
where
    FnType: FnOnce() -> Out,
{
    phantom: PhantomData<fn() -> Out>,
    fn_impl: FnType,
}

impl<FnType, Out> NoArgClosure<FnType, Out>
where
    FnType: FnOnce() -> Out,
{
    pub fn new(fn_impl: FnType) -> Self {
        Self {
            fn_impl,
            phantom: PhantomData,
        }
    }
}

impl<FnType, Out> NoArgFunctor for NoArgClosure<FnType, Out>
where
    FnType: FnOnce() -> Out,
{
    type Output = Out;

    fn tuple_invoke(self) -> Self::Output {
        (self.fn_impl)()
    }
}

/// Functor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct Closure<FnType, Out, Args>
where
    FnType: FnOnce(Args) -> Out,
{
    phantom: PhantomData<fn(Args) -> Out>,
    fn_impl: FnType,
}

impl<FnType, Out, Args> Closure<FnType, Out, Args>
where
    FnType: FnOnce(Args) -> Out,
{
    pub fn new(fn_impl: FnType) -> Closure<FnType, Out, Args> {
        Closure {
            phantom: PhantomData,
            fn_impl,
        }
    }
}

impl<FnType, Out, Args> Functor<Args> for Closure<FnType, Out, Args>
where
    FnType: FnOnce(Args) -> Out,
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
pub struct BiClosure<FnType, Out, FirstArg, Args>
where
    FnType: FnOnce(FirstArg, Args) -> Out,
{
    phantom: PhantomData<fn(FirstArg, Args) -> Out>,
    fn_impl: FnType,
}

impl<FnType, Out, FirstArg, Args> BiClosure<FnType, Out, FirstArg, Args>
where
    FnType: FnOnce(FirstArg, Args) -> Out,
{
    pub fn new(fn_impl: FnType) -> BiClosure<FnType, Out, FirstArg, Args> {
        BiClosure {
            phantom: PhantomData,
            fn_impl,
        }
    }
}

impl<FnType, Out, FirstArg, Args> BiFunctor<FirstArg, Args>
    for BiClosure<FnType, Out, FirstArg, Args>
where
    FnType: FnOnce(FirstArg, Args) -> Out,
{
    type Output = Out;

    fn tuple_invoke(self, first_arg: FirstArg, args: Args) -> Self::Output {
        let fn_impl = self.fn_impl;
        fn_impl(first_arg, args)
    }
}

/// Wrapper for functors that don't return a [Result].
/// This wrapper wraps the functor into something that will return a [Result].
pub struct NoErrNoArgFunctor<FunctorType, Out>
where
    FunctorType: NoArgFunctor<Output = Out>,
{
    functor: FunctorType,
    phantom: PhantomData<fn() -> Out>,
}

impl<FunctorType, Out> NoErrNoArgFunctor<FunctorType, Out>
where
    FunctorType: NoArgFunctor<Output = Out>,
{
    pub fn new(functor: FunctorType) -> Self {
        Self {
            functor,
            phantom: PhantomData,
        }
    }
}

impl<FunctorType, Out> NoArgFunctor for NoErrNoArgFunctor<FunctorType, Out>
where
    FunctorType: NoArgFunctor<Output = Out>,
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
pub struct NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
{
    functor: FunctorType,
    phantom: PhantomData<fn(ArgTuple) -> Out>,
}

impl<FunctorType, Out, ArgTuple> NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
{
    pub fn new(functor: FunctorType) -> NoErrFunctor<FunctorType, Out, ArgTuple> {
        NoErrFunctor {
            functor,
            phantom: PhantomData,
        }
    }
}

impl<FunctorType, Out, ArgTuple> Functor<ArgTuple> for NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
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
pub struct NoErrBiFunctor<FunctorType, Out, FirstArg, ArgTuple>
where
    FunctorType: BiFunctor<FirstArg, ArgTuple, Output = Out>,
{
    functor: FunctorType,
    phantom: PhantomData<fn(FirstArg, ArgTuple) -> Out>,
}

impl<FunctorType, Out, FirstArg, ArgTuple> NoErrBiFunctor<FunctorType, Out, FirstArg, ArgTuple>
where
    FunctorType: BiFunctor<FirstArg, ArgTuple, Output = Out>,
{
    pub fn new(functor: FunctorType) -> NoErrBiFunctor<FunctorType, Out, FirstArg, ArgTuple> {
        NoErrBiFunctor {
            functor,
            phantom: PhantomData,
        }
    }
}

impl<FunctorType, Out, FirstArg, ArgTuple> BiFunctor<FirstArg, ArgTuple>
    for NoErrBiFunctor<FunctorType, Out, FirstArg, ArgTuple>
where
    FunctorType: BiFunctor<FirstArg, ArgTuple, Output = Out>,
{
    type Output = Result<Out>;

    fn tuple_invoke(self, first_arg: FirstArg, args: ArgTuple) -> Self::Output {
        Ok(self.functor.tuple_invoke(first_arg, args))
    }
}
