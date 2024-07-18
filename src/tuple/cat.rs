/// Implement tuple concatenation.
///
/// This trait is implemented for any pair of tuples.
/// ```
/// use senders_receivers::tuple::TupleCat;
///
/// let x = (1, 2, 3);
/// let y = (4, 5, 6);
/// let x_and_y = (x, y).cat();
/// ```
///
/// Note: due to rust not having variadic-arguments,
/// we implement this using a macro,
/// and thus the implementation only exists for up-to-16 elements.
///
/// Let me know if you need more, it's trivial to expand the size.
pub trait TupleCat {
    /// Result of the concatenation.
    type Output;
    /// Perform the concatenation.
    fn cat(self) -> Self::Output;
}

macro_rules! tuple_impls_2_ {
    // Expansion sentinel.
    ($macro:ident EXPAND ()) => {
        tuple_impls_2_!($macro () ());
    };
    // Expansion.
    ($macro:ident EXPAND ($v:ident : $T:ident $($tail_v:ident : $TailT:ident)*)) => {
        tuple_impls_2_!($macro EXPAND ($($tail_v : $TailT)*));
        tuple_impls_2_!($macro () ($v : $T $($tail_v : $TailT)*));
    };
    ($macro:ident ($($w:ident : $U:ident)*) ()) => {
        $macro!(($($w : $U)*), ());
    };
    ($macro:ident ($($w:ident : $U:ident)*) ($v:ident : $T:ident $($tail_v:ident : $TailT:ident)*)) => {
        $macro!(($($w : $U)*), ($v : $T $($tail_v : $TailT)*));
        tuple_impls_2_!($macro ($($w : $U)* $v : $T) ($($tail_v : $TailT)*));
    };
}

macro_rules! tuple_impls_2 {
    ($macro:ident) => {
        tuple_impls_2_!($macro
            EXPAND
            (v1: T1  v2: T2  v3: T3  v4: T4  v5: T5  v6: T6  v7: T7  v8: T8  v9: T9  v10: T10  v11: T11  v12: T12  v13: T13  v14: T14  v15: T15  v16: T16));
    };
}

macro_rules! implement_tuple_cat {
    // 0-ary and 0-ary
    ((), ()) => {
        impl TupleCat for ((), ()) {
            type Output = ();
            fn cat(self) -> Self::Output {

            }
        }
    };
    // 0-ary and 1-ary
    ((), ($w:ident : $U:ident)) => {
        impl<$U> TupleCat for ((), ($U,)) {
            type Output = ($U,);
            fn cat(self) -> Self::Output {
                self.1
            }
        }
    };
    (($v:ident : $T:ident), ()) => {
        impl<$T> TupleCat for (($T,), ()) {
            type Output = ($T,);
            fn cat(self) -> Self::Output {
                self.0
            }
        }
    };
    // 1-ary and 1-ary
    (($v:ident : $T:ident), ($w:ident : $U:ident)) => {
        impl<$T, $U> TupleCat for (($T,), ($U,)) {
            type Output = ($T, $U);
            fn cat(self) -> Self::Output {
                let ($v,) = self.0;
                let ($w,) = self.1;
                ($v, $w)
            }
        }
    };
    // 0-ary and n-ary
    ((), ($($w:ident : $U:ident)+)) => {
        impl<$($U),+> TupleCat for ((), ($($U),+)) {
            type Output = ($($U),+);
            fn cat(self) -> Self::Output {
                self.1
            }
        }
    };
    (($($v:ident : $T:ident)+), ()) => {
        impl<$($T),+> TupleCat for (($($T),+), ()) {
            type Output = ($($T),+);
            fn cat(self) -> Self::Output {
                self.0
            }
        }
    };
    // 1-ary and n-ary
    (($v:ident : $T:ident), ($($w:ident : $U:ident)+)) => {
        impl<$T, $($U),+> TupleCat for (($T,), ($($U),+)) {
            type Output = ($T, $($U),+);
            fn cat(self) -> Self::Output {
                let ($v,) = self.0;
                let ($($w),+) = self.1;
                ($v, $($w),+)
            }
        }
    };
    (($($v:ident : $T:ident)+), ($w:ident : $U:ident)) => {
        impl<$($T),+, $U> TupleCat for (($($T),+), ($U,)) {
            type Output = ($($T),+, $U);
            fn cat(self) -> Self::Output {
                let ($($v),+) = self.0;
                let ($w,) = self.1;
                ($($v),+, $w)
            }
        }
    };
    // n-ary and n-ary
    (($($v:ident : $T:ident)+), ($($w:ident : $U:ident)+)) => {
        impl<$($T),+, $($U),+> TupleCat for (($($T),+), ($($U),+)) {
            type Output = ($($T),+, $($U),+);
            fn cat(self) -> Self::Output {
                let ($($v),+) = self.0;
                let ($($w),+) = self.1;
                ($($v),+, $($w),+)
            }
        }
    };
}

tuple_impls_2!(implement_tuple_cat);
