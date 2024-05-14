use crate::errors::IsTuple;
use std::marker::PhantomData;

pub trait Functor<Args>
where
    Args: IsTuple,
{
    type Output;

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

//macro_rules! tuple_invoke {
//    ($($i:ident)*) => {
//        impl<FnType, Out, $($i),*> Functor<($($i),*)> for Closure<FnType, Out, $($i),*>
//        where
//            FnType: FnOnce($($i),*) -> Out
//        {
//            type Output = Out;
//
//            fn invoke(self, ($(concat_idents!(value_for_, $i)),*): ($($i),*)) -> Self::Output {
//                let fn_impl = self.fn_impl;
//                fn_impl($(concat_idents!(value_for_, $i)),*)
//            }
//        }
//    };
//}
//
//crate::errors::tuple_impls!(tuple_invoke);
