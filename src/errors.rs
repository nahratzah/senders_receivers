use core::any::Any;

pub trait IsResult {
    type Type;
    type Error;
}
pub trait IsTuple {}

impl<T, E> IsResult for Result<T, E> {
    type Type = T;
    type Error = E;
}

// Copying the trick from https://doc.rust-lang.org/src/core/tuple.rs.html to implement lots of implementations.
macro_rules! tuple_impls_ {
    // Stopping critera (0-ary tuple)
    ($macro:ident) => {
        $macro!();
    };
    // Running criteria (1-ary tuple)
    ($macro:ident $T:ident) => {
        tuple_impls_!($macro);
        $macro!($T);
    };
    // Running criteria (n-ary tuple)
    ($macro:ident $T:ident $($U:ident)+) => {
        tuple_impls_!($macro $($U)+);
        $macro!($T $($U)+);
    };
}

macro_rules! make_is_tuple {
    () => {
        impl IsTuple for () {}
    };
    ($T:ident) => {
        impl<$T> IsTuple for ($T,) {}
    };
    ($T:ident $($U:ident)+) => {
        impl<$T, $($U),+> IsTuple for ($T, $($U),+) {}
    };
}

macro_rules! tuple_impls {
    ($macro:ident) => {
	tuple_impls_!($macro T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);
    };
}

tuple_impls!(make_is_tuple);

pub type Error = Box<dyn Any>;

pub struct NoError;
