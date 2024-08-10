use super::{StopCallback, StopToken};

/// A [StopToken] that cannot be marked `stopped`.
#[derive(Clone, Debug)]
pub struct NeverStopToken;

impl StopToken for NeverStopToken {
    const STOP_POSSIBLE: bool = false;

    fn stop_requested(&self) -> bool {
        false
    }

    type CallbackType = NeverStopCallback;

    fn callback<F>(&self, _: F) -> Result<Self::CallbackType, F>
    where
        F: 'static + Send + FnOnce(),
    {
        Ok(NeverStopCallback)
    }

    fn detached_callback<F>(&self, _: F) -> Result<(), F>
    where
        F: 'static + Send + FnOnce(),
    {
        Ok(())
    }
}

#[derive(Debug)]
pub struct NeverStopCallback;

impl Default for NeverStopCallback {
    fn default() -> Self {
        NeverStopCallback
    }
}

impl StopCallback for NeverStopCallback {
    fn detach(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::{NeverStopToken, StopToken};

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn it_works() {
        assert!(!NeverStopToken::STOP_POSSIBLE);
        assert!(!NeverStopToken.stop_requested());
        assert!(NeverStopToken.callback(|| ()).is_ok());
    }
}
