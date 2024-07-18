// Copying the trick from https://doc.rust-lang.org/src/core/tuple.rs.html to implement lots of implementations.
macro_rules! tuple_impls_ {
    // Stopping critera (0-ary tuple)
    ($macro:ident) => {
        $macro!();
    };
    // Running criteria (1-ary tuple)
    ($macro:ident, $v:ident : $T:ident) => {
        crate::tuple::macros::tuple_impls_!($macro);
        $macro!($v: $T);
    };
    // Running criteria (n-ary tuple)
    ($macro:ident, $v:ident : $T:ident , $($tail_v:ident : $TailT:ident),*) => {
        crate::tuple::macros::tuple_impls_!($macro, $($tail_v: $TailT),*);
        $macro!($v: $T, $($tail_v: $TailT),*);
    };
}
pub(crate) use tuple_impls_;

/// This macro invokes another macro repeatedly, each time with one more `v: T` pair.
/// This is used to generate tuple specializations.
macro_rules! tuple_impls {
    ($macro:ident) => {
        crate::tuple::macros::tuple_impls_!($macro, v1: T1, v2: T2, v3: T3, v4: T4, v5: T5, v6: T6, v7: T7, v8: T8, v9: T9, v10: T10, v11: T11, v12: T12, v13: T13, v14: T14, v15: T15, v16: T16);
    };
}
pub(crate) use tuple_impls;
