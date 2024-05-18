use std::error;

pub trait IsTuple {}

// Copying the trick from https://doc.rust-lang.org/src/core/tuple.rs.html to implement lots of implementations.
macro_rules! tuple_impls_ {
    // Stopping critera (0-ary tuple)
    ($macro:ident) => {
        $macro!();
    };
    // Running criteria (1-ary tuple)
    ($macro:ident $T:ident) => {
        crate::errors::tuple_impls_!($macro);
        $macro!($T);
    };
    // Running criteria (n-ary tuple)
    ($macro:ident $T:ident $($U:ident)+) => {
        crate::errors::tuple_impls_!($macro $($U)+);
        $macro!($T $($U)+);
    };
}
pub(crate) use tuple_impls_;

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
        crate::errors::tuple_impls_!($macro T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);
    };
}
pub(crate) use tuple_impls;

tuple_impls!(make_is_tuple);

pub type Error = Box<dyn error::Error>;

#[cfg(test)]
mod for_testing {
    use std::error;
    use std::fmt;

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
