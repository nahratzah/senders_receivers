//! The `functors` mod holds functions.
//!
//! The reason we need those, is because we need to have composible functions.
//! The `Functor` trait provides something we can invoke.

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

/// Composition functor.
///
/// A composition functor takes two functors (`f` and `g`, and creates a new functor `n`, such that: `n(x) -> g(f(x))`.
struct Composition<FirstFn, NextFn, Args>
where
    Args: IsTuple,
    FirstFn: Functor<Args>,
    NextFn: Functor<(<FirstFn as Functor<Args>>::Output,)>,
{
    phantom: PhantomData<Args>,
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

/// Functor for a function/closure.
///
/// The functor will delegate to the contained function.
pub struct Closure<FnType, Out, Args>
where
    Args: IsTuple,
    FnType: FnOnce(Args) -> Out,
{
    phantom_args: PhantomData<Args>,
    phantom_out: PhantomData<Out>,
    fn_impl: FnType,
}

impl<FnType, Out, Args> Closure<FnType, Out, Args>
where
    Args: IsTuple,
    FnType: FnOnce(Args) -> Out,
{
    pub fn new(fn_impl: FnType) -> Closure<FnType, Out, Args>
    where
        Args: IsTuple,
        FnType: FnOnce(Args) -> Out,
    {
        Closure {
            phantom_args: PhantomData,
            phantom_out: PhantomData,
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

// If we had `Fn`-family traits, we could do this:
//
// ```
// pub struct XClosure<FnType, Out, Args>
// where
//     Args: IsTuple+std::marker::Tuple,
//     FnType: FnOnce<Args, Output=Out>,
// {
//     phantom_args: PhantomData<Args>,
//     phantom_out: PhantomData<Out>,
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

/// Wrapper for functors that don't return a `Result`.
/// This wrapper wraps the functor into something that will return a `Result`.
///
/// We need to use an actual struct, because we need to declare types.
/// With closures, we cannot capture the closure type, and thus not create a specialization.
pub struct NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
{
    functor: FunctorType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<FunctorType, Out, ArgTuple> NoErrFunctor<FunctorType, Out, ArgTuple>
where
    FunctorType: Functor<ArgTuple, Output = Out>,
    ArgTuple: IsTuple,
{
    pub fn new(functor: FunctorType) -> NoErrFunctor<FunctorType, Out, ArgTuple> {
        NoErrFunctor {
            functor,
            phantom1: PhantomData,
            phantom2: PhantomData,
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
