use super::{StopToken, StopTokenCallback};

/// A [StopToken] that cannot be marked `stopped`.
#[derive(Clone, Debug)]
pub struct NeverStopToken;

impl StopToken for NeverStopToken {
    const STOP_POSSIBLE: bool = false;

    fn stop_requested(&self) -> bool {
        false
    }
}

impl<F> StopTokenCallback<F> for NeverStopToken
where
    F: 'static + FnOnce(),
{
    type CallbackType = NeverStopCallback;

    fn callback(&self, _: F) -> Result<Self::CallbackType, F> {
        Ok(NeverStopCallback)
    }
}

#[derive(Debug)]
pub struct NeverStopCallback;

#[cfg(test)]
mod tests {
    use super::{NeverStopToken, StopToken, StopTokenCallback};

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn it_works() {
        assert!(!NeverStopToken::STOP_POSSIBLE);
        assert!(!NeverStopToken.stop_requested());
        assert!(NeverStopToken.callback(|| ()).is_ok());
    }
}
