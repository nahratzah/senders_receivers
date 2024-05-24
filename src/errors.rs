use std::error;

/// Confirm if a type is a tuple.
pub trait IsTuple {}

// Copying the trick from https://doc.rust-lang.org/src/core/tuple.rs.html to implement lots of implementations.
macro_rules! tuple_impls_ {
    // Stopping critera (0-ary tuple)
    ($macro:ident) => {
        $macro!();
    };
    // Running criteria (1-ary tuple)
    ($macro:ident, $v:ident : $T:ident) => {
        crate::errors::tuple_impls_!($macro);
        $macro!($v: $T);
    };
    // Running criteria (n-ary tuple)
    ($macro:ident, $v:ident : $T:ident , $($tail_v:ident : $TailT:ident),*) => {
        crate::errors::tuple_impls_!($macro, $($tail_v: $TailT),*);
        $macro!($v: $T, $($tail_v: $TailT),*);
    };
}
pub(crate) use tuple_impls_;

macro_rules! make_is_tuple {
    () => {
        impl IsTuple for () {}
    };
    ($v:ident : $T:ident) => {
        impl<$T> IsTuple for ($T,) {}
    };
    ($($v:ident: $T:ident),+) => {
        impl<$($T),+> IsTuple for ($($T),+) {}
    };
}

/// This macro invokes another macro repeatedly, each time with one more `v: T` pair.
/// This is used to generate tuple specializations.
macro_rules! tuple_impls {
    ($macro:ident) => {
        crate::errors::tuple_impls_!($macro, v1: T1, v2: T2, v3: T3, v4: T4, v5: T5, v6: T6, v7: T7, v8: T8, v9: T9, v10: T10, v11: T11, v12: T12, v13: T13, v14: T14, v15: T15, v16: T16);
    };
}
pub(crate) use tuple_impls;

tuple_impls!(make_is_tuple);

/// Errors as passed as opaque types.
///
/// If you want to handle specific errors, you can downcast the relevant error to a type.
/// The [new_error] function is provided to more easily convert an error into an [Error].
pub type Error = Box<dyn error::Error + Send>;

/// Create a new error from something that looks like an error.
pub fn new_error<E>(e: E) -> Error
where
    E: error::Error + Send + 'static + Sized,
{
    Box::new(e)
}

#[cfg(test)]
mod for_testing {
    use std::error;
    use std::fmt;

    /// An implementation of Error, used during testing.
    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct Error {
        text: String,
    }

    impl Error {
        pub fn new(text: String) -> Error {
            Error { text }
        }

        pub fn from(text: &str) -> Error {
            Self::new(String::from(text))
        }
    }

    impl error::Error for Error {}

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.text)
        }
    }
}

#[cfg(test)]
pub type ErrorForTesting = for_testing::Error;
