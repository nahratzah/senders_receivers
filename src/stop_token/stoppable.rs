use super::{StopCallback, StopToken};
use std::backtrace::Backtrace;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

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

impl<StopTokenImpl> From<StopTokenImpl> for StopSource
where
    StopTokenImpl: StopToken,
{
    /// Create a new [StopSource], and make it so it'll be stopped when the [StopToken] is stopped.
    fn from(stop_token: StopTokenImpl) -> Self {
        let new_stop_source = Self::default();
        if StopTokenImpl::STOP_POSSIBLE {
            let new_stop_source = new_stop_source.clone();
            if let Err(f) = stop_token.detached_callback(move || new_stop_source.request_stop()) {
                f(); // stop_token is already stopped, so just stop the new stop source (by running the function).
            };
        }
        new_stop_source
    }
}

impl StopSource {
    /// Request that any sender-chains be stopped.
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

    type CallbackType = StoppableCallback;

    fn callback<F>(&self, f: F) -> Result<Self::CallbackType, F>
    where
        F: 'static + Send + FnOnce(),
    {
        StoppableCallback::new(&self.state, f)
    }
}

pub struct StoppableCallback {
    state: Weak<StopSourceState>,
    callback_slot: usize,
}

impl StoppableCallback {
    fn new<F>(state: &Arc<StopSourceState>, f: F) -> Result<Self, F>
    where
        F: 'static + Send + FnOnce(),
    {
        let stacktrace = Backtrace::capture();
        state.register(f, stacktrace).map(|callback_slot| Self {
            state: Arc::downgrade(state),
            callback_slot,
        })
    }
}

impl StopCallback for StoppableCallback {
    fn detach(&mut self) {
        self.state = Weak::new();
        self.callback_slot = usize::MAX;
    }
}

impl Default for StoppableCallback {
    fn default() -> Self {
        Self {
            state: Weak::new(),
            callback_slot: usize::MAX,
        }
    }
}

impl Drop for StoppableCallback {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state.deregister(self.callback_slot);
        }
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

    fn deregister(&self, callback_slot: usize) {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks[callback_slot].clear();
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
    use super::{StopSource, StopToken};
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn token_reflects_stopsource_state() {
        let source = StopSource::default();
        let token = source.token();

        assert!(get_stop_possible(&token));
        assert!(!token.stop_requested(), "source should not be stopped");

        req_stop_in_thread(&source);
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

        req_stop_in_thread(&source);
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

        req_stop_in_thread(&source);
        assert!(!cb_state.is_called(), "should not be called at all, because we deregistered the callback prior to requesting stop");
    }

    /// Request the stop in another thread, so that Send will complain if we lack it.
    fn req_stop_in_thread(source: &StopSource) {
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
