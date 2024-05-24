//! The `functors` mod holds functions.
//!
//! The reason we need those, is because we need to have composible functions.
//! The [Functor] and [BiFunctor] traits provide something we can invoke.
//!
//! Note: we might erase this module in the future, if implementing `FnOnce` becomes doable.

use crate::errors::{Error, IsTuple};
use std::marker::PhantomData;

/// A functor can be invoked.
///
/// Args represents a tuple of arguments.
pub trait Functor<Args>
where
    Args: IsTuple,
{
    /// The result type of the functor.
    type Output;

    /// Invoke this functor.
    ///
    /// The function name is `tuple_invoke`, because:
    /// - we invoke it with tuple only;
    /// - I wanted a name that wouldn't clash, if in the future variadic traits became a thing.
    fn tuple_invoke(self, args: Args) -> Self::Output;

    fn then(self, next_fn: impl Functor<(Self::Output,)>) -> impl Functor<Args>
    where
        Self: Sized,
    {
        Composition {
            phantom: PhantomData,
            first_fn: self,
            next_fn,
        }
    }
}

/// A bi-functor can be invoked with two arguments.
///
/// The second argument represents a tuple of arguments.
pub trait BiFunctor<FirstArg, Args>
where
    Args: IsTuple,
{
    /// The result type of the functor.
    type Output;

    /// Invoke this functor.
    fn tuple_invoke(self, first_arg: FirstArg, args: Args) -> Self::Output;
}

/// Composition functor.
///
/// A composition functor takes two functors (`f` and `g`, and creates a new functor `n`, such that: `n(x) -> g(f(x))`.
struct Composition<FirstFn, NextFn, Args>
where
    Args: IsTuple,
    FirstFn: Functor<Args>,
    NextFn: Functor<(<FirstFn as Functor<Args>>::Output,)>,
{
    phantom: PhantomData<fn(Args)>,
    first_fn: FirstFn,
    next_fn: NextFn,
}

impl<FirstFn, NextFn, Args> Functor<Args> for Composition<FirstFn, NextFn, Args>
where
    Args: IsTuple,
    FirstFn: Functor<Args>,
    NextFn: Functor<(FirstFn::Output,)>,
{
    type Output = NextFn::Output;

    fn tuple_invoke(self, args: Args) -> Self::Output {
        self.next_fn
            .tuple_invoke((self.first_fn.tuple_invoke(args),))
    }
}

/// Functor for a [function/closure](FnOnce).
///
/// The functor will delegate to the contained function.
pub struct Closure<FnType, Out, Args>
where
    Args: IsTuple,
    FnType: FnOnce(Args) -> Out,
{
    phantom: PhantomData<fn(Args) -> Out>,
    fn_impl: FnType,
}

impl<FnType, Out, Args> Closure<FnType, Out, Args>
where
    Args: IsTuple,
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
    Args: IsTuple,
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
    Args: IsTuple,
    FnType: FnOnce(FirstArg, Args) -> Out,
{
    phantom: PhantomData<fn(FirstArg, Args) -> Out>,
    fn_impl: FnType,
}

impl<FnType, Out, FirstArg, Args> BiClosure<FnType, Out, FirstArg, Args>
where
    Args: IsTuple,
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
    Args: IsTuple,
    FnType: FnOnce(FirstArg, Args) -> Out,
{
    type Output = Out;

    fn tuple_invoke(self, first_arg: FirstArg, args: Args) -> Self::Output {
        let fn_impl = self.fn_impl;
        fn_impl(first_arg, args)
    }
}

// If we had `Fn`-family traits, we could do this:
//
// ```
// pub struct XClosure<FnType, Out, Args>
// where
//     Args: IsTuple+std::marker::Tuple,
//     FnType: FnOnce<Args, Output=Out>,
// {
//     phantoms: PhantomData<fn(Args) -> Out>,
//     fn_impl: FnType,
// }
//
// macro_rules! closure_invoke {
//     () => {
//         impl<FnType, Out> Functor<()> for XClosure<FnType, Out, ()>
//         where
//             (): IsTuple,
//             FnType: FnOnce() -> Out
//         {
//             type Output = Out;
//
//             fn tuple_invoke(self, _: ()) -> Self::Output {
//                 let fn_impl = self.fn_impl;
//                 fn_impl()
//             }
//         }
//     };
//     ($i:ident : $T:ident) => {
//         impl<FnType, Out, $T> Functor<($T,)> for XClosure<FnType, Out, ($T,)>
//         where
//             (): IsTuple,
//             FnType: FnOnce($T) -> Out
//         {
//             type Output = Out;
//
//             fn tuple_invoke(self, $i: ($T,)) -> Self::Output {
//                 let fn_impl = self.fn_impl;
//                 fn_impl($i)
//             }
//         }
//     };
//     ($i:ident : $T:ident) => {
//         panic!("Please don't!");
//     };
//     ($($i:ident : $T:ident),*) => {
//         impl<FnType, Out, $($T),*> Functor<($($T),*)> for XClosure<FnType, Out, ($($T),*)>
//         where
//             ($($T),*): IsTuple,
//             FnType: FnOnce($($T),*) -> Out
//         {
//             type Output = Out;
//
//             fn tuple_invoke(self, ($($i),*): ($($T),*)) -> Self::Output {
//                 let fn_impl = self.fn_impl;
//                 fn_impl($($i),*)
//             }
//         }
//     };
// }
//
// crate::errors::tuple_impls!(closure_invoke);
// ```

/// Wrapper for functors that don't return a [Result].
/// This wrapper wraps the functor into something that will return a [Result].
///
/// We need to use an actual struct, because we need to declare types.
/// With closures, we cannot capture the closure type, and thus not create a specialization.
pub struct NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
{
    functor: FunctorType,
    phantom: PhantomData<fn(ArgTuple) -> Out>,
}

impl<FunctorType, Out, ArgTuple> NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
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
    ArgTuple: IsTuple,
{
    type Output = Result<Out, Error>;

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
    ArgTuple: IsTuple,
{
    functor: FunctorType,
    phantom: PhantomData<fn(FirstArg, ArgTuple) -> Out>,
}

impl<FunctorType, Out, FirstArg, ArgTuple> NoErrBiFunctor<FunctorType, Out, FirstArg, ArgTuple>
where
    FunctorType: BiFunctor<FirstArg, ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
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
    ArgTuple: IsTuple,
{
    type Output = Result<Out, Error>;

    fn tuple_invoke(self, first_arg: FirstArg, args: ArgTuple) -> Self::Output {
        Ok(self.functor.tuple_invoke(first_arg, args))
    }
}
