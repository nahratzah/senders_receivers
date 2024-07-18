use super::macros::tuple_impls;

/// Distribute references to a tuple.
///
/// This trait allows a reference-to-tuple, to be turned into a tuple-of-references.
/// ```
/// use senders_receivers::tuple::DistributeRefTuple;
///
/// let tpl = (3_i32, 4_i32, 5_i32);
/// let ref_of_tpl = &tpl;
///
/// // x: &i32
/// // y: &i32
/// // z: &i32
/// let (x, y, z) = DistributeRefTuple::distribute(ref_of_tpl);
/// ```
pub trait DistributeRefTuple {
    /// Result of the distribute operation.
    type Output;

    /// Perform the distribute operation.
    fn distribute(_: Self) -> Self::Output;
}

macro_rules! make_distribute_ref_tuple {
    () => {
        impl DistributeRefTuple for &mut () {
            type Output = ();

            fn distribute(_: Self) -> Self::Output {

            }
        }

        impl DistributeRefTuple for &() {
            type Output = ();

            fn distribute(_: Self) -> Self::Output {

            }
        }
    };
    ($v:ident : $T:ident) => {
        impl<'a, $T> DistributeRefTuple for &'a mut ($T,) {
            type Output = (&'a mut $T,);

            fn distribute(x: Self) -> Self::Output {
                let ($v,) = x;
                ($v,)
            }
        }

        impl<'a, $T> DistributeRefTuple for &'a ($T,) {
            type Output = (&'a $T,);

            fn distribute(x: Self) -> Self::Output {
                let ($v,) = x;
                ($v,)
            }
        }
    };
    ($($v:ident : $T:ident),+) => {
        impl<'a, $($T),+> DistributeRefTuple for &'a mut ($($T),+) {
            type Output = ($(&'a mut $T),+);

            fn distribute(x: Self) -> Self::Output {
                let ($($v),+) = x;
                ($($v),+)
            }
        }

        impl<'a, $($T),+> DistributeRefTuple for &'a ($($T),+) {
            type Output = ($(&'a $T),+);

            fn distribute(x: Self) -> Self::Output {
                let ($($v),+) = x;
                ($($v),+)
            }
        }
    };
}

tuple_impls!(make_distribute_ref_tuple);
