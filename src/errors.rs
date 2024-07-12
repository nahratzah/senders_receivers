use std::error;

/// Errors are passed as opaque types.
///
/// If you want to handle specific errors, you can downcast the relevant error to a type.
/// The [new_error] function is provided to more easily convert an error into an [Error].
pub type Error = Box<dyn error::Error + Send>;

/// Result type used in senders/receivers.
pub type Result<T> = std::result::Result<T, Error>;

/// Create a new error from something that looks like an error.
pub fn new_error<E>(e: E) -> Error
where
    E: error::Error + Send + 'static + Sized,
{
    Box::new(e)
}

#[cfg(test)]
pub type ErrorForTesting = for_testing::Error;

#[cfg(test)]
mod for_testing {
    use std::error;
    use std::fmt;

    /// An implementation of Error, used during testing.
    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct Error {
        text: String,
    }

    impl From<&'static str> for Error {
        fn from(text: &'static str) -> Self {
            Self::from(String::from(text))
        }
    }

    impl From<String> for Error {
        fn from(text: String) -> Self {
            Error { text }
        }
    }

    impl error::Error for Error {}

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.text)
        }
    }
}
