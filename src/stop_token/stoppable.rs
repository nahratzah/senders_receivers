use super::{StopToken, StopTokenCallback};
use std::backtrace::Backtrace;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A source for a [StopToken].
///
/// The source is shared between stop-tokens.
#[derive(Clone, Debug)]
pub struct StopSource {
    state: Arc<StopSourceState>,
}

impl Default for StopSource {
    fn default() -> Self {
        Self {
            state: Arc::new(StopSourceState::default()),
        }
    }
}

impl StopSource {
    /// Request that any sender-chains be canceled.
    pub fn request_stop(&self) {
        self.state.request_stop()
    }

    /// Get the [StopToken] for this.
    pub fn token(&self) -> StoppableToken {
        StoppableToken {
            state: self.state.clone(),
        }
    }
}

/// Stop token used for [StopSource].
#[derive(Clone, Debug)]
pub struct StoppableToken {
    state: Arc<StopSourceState>,
}

impl StopToken for StoppableToken {
    const STOP_POSSIBLE: bool = true;

    fn stop_requested(&self) -> bool {
        self.state.stop_requested()
    }
}

impl<F> StopTokenCallback<F> for StoppableToken
where
    F: 'static + Send + FnOnce(),
{
    type CallbackType = StoppableCallback;

    fn callback(&self, f: F) -> Result<Self::CallbackType, F> {
        StoppableCallback::new(&self.state, f)
    }
}

pub struct StoppableCallback {
    state: Arc<StopSourceState>,
    cancelation_slot: usize,
}

impl StoppableCallback {
    fn new<F>(state: &Arc<StopSourceState>, f: F) -> Result<Self, F>
    where
        F: 'static + Send + FnOnce(),
    {
        let stacktrace = Backtrace::capture();
        state.register(f, stacktrace).map(|cancelation_slot| Self {
            state: state.clone(),
            cancelation_slot,
        })
    }
}

impl Drop for StoppableCallback {
    fn drop(&mut self) {
        self.state.deregister(self.cancelation_slot);
    }
}

#[derive(Debug)]
struct StopSourceState {
    stopped: AtomicBool,
    callbacks: Mutex<Vec<Box<dyn CallbackWrapper>>>,
}

impl StopSourceState {
    fn request_stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        for callback in &mut *self.callbacks.lock().unwrap() {
            callback.invoke();
        }
    }

    fn stop_requested(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    fn register<F>(&self, f: F, stacktrace: Backtrace) -> Result<usize, F>
    where
        F: 'static + Send + FnOnce(),
    {
        let mut callbacks = self.callbacks.lock().unwrap();
        if self.stopped.load(Ordering::Relaxed) {
            return Err(f);
        }

        let index = callbacks.len();
        callbacks.push(Box::new(CallbackWrapperImpl::new(f, stacktrace)));
        Ok(index)
    }

    fn deregister(&self, cancelation_slot: usize) {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks[cancelation_slot].clear();
    }
}

impl Default for StopSourceState {
    fn default() -> Self {
        Self {
            stopped: AtomicBool::new(false),
            callbacks: Mutex::new(Vec::default()),
        }
    }
}

trait CallbackWrapper: fmt::Debug + Send {
    fn invoke(&mut self);
    fn clear(&mut self);
}

struct CallbackWrapperImpl<F>(Option<F>, Backtrace)
where
    F: 'static + Send + FnOnce();

impl<F> CallbackWrapperImpl<F>
where
    F: 'static + Send + FnOnce(),
{
    fn new(f: F, stacktrace: Backtrace) -> Self {
        Self(Some(f), stacktrace)
    }

    fn move_functor(&mut self) -> Option<F> {
        self.0.take()
    }
}

impl<F> CallbackWrapper for CallbackWrapperImpl<F>
where
    F: 'static + Send + FnOnce(),
{
    fn invoke(&mut self) {
        if let Some(f) = self.move_functor() {
            f()
        }
    }

    fn clear(&mut self) {
        drop(self.move_functor())
    }
}

impl<F> fmt::Debug for CallbackWrapperImpl<F>
where
    F: 'static + Send + FnOnce(),
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct FakeNone;
        impl fmt::Debug for FakeNone {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("None")
            }
        }

        struct FakeSomething;
        impl fmt::Debug for FakeSomething {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<something>")
            }
        }

        let mut debug_struct = f.debug_struct("StopTokenCallback");
        if self.0.is_some() {
            debug_struct.field("f", &FakeSomething);
        } else {
            debug_struct.field("f", &FakeNone);
        }
        debug_struct
            .field("stacktrace-at-construction", &self.1)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{StopSource, StopToken, StopTokenCallback};
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn token_reflects_stopsource_state() {
        let source = StopSource::default();
        let token = source.token();

        assert!(get_stop_possible(&token));
        assert!(!token.stop_requested(), "source should not be stopped");

        req_cancel_in_thread(&source);
        assert!(
            token.stop_requested(),
            "source should be marked stopped, now that we've requested it"
        );
    }

    #[test]
    fn callback_gets_called() {
        let source = StopSource::default();
        let token = source.token();

        let cb_state = Called::new();
        let cb = token.callback({
            let cb_state = cb_state.clone();
            move || {
                cb_state.mark_called();
            }
        });

        assert!(
            !cb_state.is_called(),
            "should not yet be called, because the source isn't stopped"
        );

        req_cancel_in_thread(&source);
        assert!(
            cb_state.is_called(),
            "should have been called, because we requested stop"
        );

        drop(cb);
    }

    #[test]
    fn dropped_callback_does_not_get_called() {
        let source = StopSource::default();
        let token = source.token();

        let cb_state = Called::new();
        let cb = token.callback({
            let cb_state = cb_state.clone();
            move || {
                cb_state.mark_called();
            }
        });

        assert!(
            !cb_state.is_called(),
            "should not yet be called, because the source isn't stopped"
        );
        drop(cb); // Drop the callback. This should deregister it.

        req_cancel_in_thread(&source);
        assert!(!cb_state.is_called(), "should not be called at all, because we deregistered the callback prior to requesting stop");
    }

    /// Request the stop in another thread, so that Send will complain if we lack it.
    fn req_cancel_in_thread(source: &StopSource) {
        let source = source.clone();
        thread::spawn(move || source.request_stop())
            .join()
            .expect("thread completes successfully")
    }

    // Retrieve [StopToken::STOP_POSSIBLE] from a variable.
    fn get_stop_possible<T>(_: &T) -> bool
    where
        T: StopToken,
    {
        T::STOP_POSSIBLE
    }

    struct Called {
        state: Mutex<bool>,
    }

    impl Called {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                state: Mutex::new(false),
            })
        }

        fn is_called(&self) -> bool {
            *self.state.lock().unwrap()
        }

        fn mark_called(&self) {
            *self.state.lock().unwrap() = true;
        }
    }
}
