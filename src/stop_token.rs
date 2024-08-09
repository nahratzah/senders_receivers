//! Stop-tokens are used to signal a sender-chain to stop processing early.
//!
//! If you come from a `go` background, this would be comparable to [Context](https://pkg.go.dev/context#Context).
//!
//! You can create a [StopSource], and then test when it's stopped:
//! ```
//! use senders_receivers::stop_token::*;
//!
//! let source = StopSource::default(); // A source can be stopped.
//!
//! let token = source.token();
//! assert!(!token.stop_requested()); // The token is used to observe the source.
//!
//! // Once the source is stopped, the token will reflect this:
//! source.request_cancel();
//! assert!(token.stop_requested());
//! ```
//!
//! A [StopSource] can also accept callbacks, to be invoked when a stop is requested:
//! ```
//! use senders_receivers::stop_token::*;
//!
//! let source = StopSource::default(); // A source can be stopped.
//!
//! let token = source.token();
//! let my_callback = token.callback(|| println!("the source was stopped"));
//!
//! source.request_cancel(); // Prints "the source was stopped".
//! ```

mod never;
mod stoppable;

pub use never::NeverStopToken;
pub use stoppable::{StopSource, StopSourceSend, StoppableToken, StoppableTokenSend};

/// A stop-token keeps track of if a sender-chain has been requested to stop.
///
/// Stop-tokens allow two ways of checking if an operation has been canceled:
/// - by inspecting the [StopToken::stop_requested] method
/// - by installing a callback that'll be invoked once a stop is requested
pub trait StopToken: Clone {
    /// Indicate if this stop-token can actually ever result in a stop being requested.
    const STOP_POSSIBLE: bool;

    /// Indicate if a stop has been requested.
    fn stop_requested(&self) -> bool;
}

/// This trait allows us to wrap callback functions for the [StopToken].
///
/// A callback is invoked when the [StopToken] transitions from not-stopped, to stopped.
pub trait StopTokenCallback<F>: StopToken
where
    F: 'static + FnOnce(),
{
    /// Callback type, that holds on to the callback function.
    ///
    /// A callback wraps a function that is invoked when the stop-token is marked as stopped.
    /// Dropping the callback will deregister it, but if a cancelation is requested on a different thread,
    /// the callback invocation may happen anyway.
    type CallbackType;

    /// Create and register a new callback.
    ///
    /// If the stop-token has already completed, an error will be returned.
    fn callback(&self, f: F) -> Result<Self::CallbackType, F>;
}
