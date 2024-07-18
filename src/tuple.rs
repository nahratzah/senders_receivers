//! Tuple utility types.
//!
//! Since we depend so much on tuples,
//! we implement and export tuple utilities here.

mod cat;
mod distribute;
pub(crate) mod macros;

pub use cat::TupleCat;
pub use distribute::DistributeRefTuple;
pub(crate) use macros::tuple_impls;

/// Confirm if a type is a tuple.
///
/// Note: once [Tuple](std::marker::Tuple) stabilizes, we'll change this to
/// `pub use std::marker::Tuple;` (with an appropriate deprecation message).
pub trait Tuple {}

macro_rules! make_is_tuple {
    () => {
        impl Tuple for () {}
    };
    ($v:ident : $T:ident) => {
        impl<$T> Tuple for ($T,) {}
    };
    ($($v:ident: $T:ident),+) => {
        impl<$($T),+> Tuple for ($($T),+) {}
    };
}

tuple_impls!(make_is_tuple);
