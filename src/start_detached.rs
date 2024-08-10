use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::scope::{detached_scope, ScopeDataSendPtr, ScopeImpl};
use crate::stop_token::{NeverStopToken, StopToken};
use crate::traits::{OperationState, Receiver, ReceiverOf, TypedSenderConnect};
use crate::tuple::Tuple;

/// The start-detached operation allows to start an operation, without waiting for its completion.
pub trait StartDetached {
    /// Start an operation, but don't wait for its completion.
    ///
    /// If the operation completes with an error signal, the code will panic.
    /// Otherwise (value or done signal), the code will complete normally.
    fn start_detached(self);
}

impl<T> StartDetached for T
where
    T: StartDetachedWithStopToken<NeverStopToken>,
{
    fn start_detached(self) {
        self.start_detached_with_stop_token(NeverStopToken)
    }
}

/// The [StartDetached] operation, but with an associated [StopToken] to allow for stopping the operation.
pub trait StartDetachedWithStopToken<StopTokenImpl>
where
    StopTokenImpl: StopToken,
{
    /// Start an operation, but don't wait for its completion.
    ///
    /// If the operation completes with an error signal, the code will panic.
    /// Otherwise (value or done signal), the code will complete normally.
    ///
    /// The provided [StopToken] can be used to request the operation to stop early.
    fn start_detached_with_stop_token(self, stop_token: StopTokenImpl);
}

impl<StopTokenImpl, T> StartDetachedWithStopToken<StopTokenImpl> for T
where
    StopTokenImpl: StopToken,
    T: TypedSenderConnect<'static, ScopeImpl<ScopeDataSendPtr>, StopTokenImpl, DiscardingReceiver>,
{
    fn start_detached_with_stop_token(self, stop_token: StopTokenImpl) {
        detached_scope(move |scope: &ScopeImpl<ScopeDataSendPtr>| {
            self.connect(scope, stop_token, DiscardingReceiver { completed: false })
                .start();
        })
    }
}

/// Discarding receiver will the done-signal or value-signal, and discard it.
///
/// It'll panic when receiver the error-signal, and also panic if it's never completed.
pub struct DiscardingReceiver {
    completed: bool,
}

impl Receiver for DiscardingReceiver {
    fn set_done(mut self) {
        self.completed = true;
    }

    fn set_error(mut self, error: Error) {
        self.completed = true;
        panic!("detached completion failed with error: {:?}", error);
    }
}

impl<Sch, Tpl> ReceiverOf<Sch, Tpl> for DiscardingReceiver
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Tpl: Tuple,
{
    fn set_value(mut self, _: Sch, _: Tpl) {
        // Since we run detached, we discard the arguments.
        self.completed = true;
    }
}

impl Drop for DiscardingReceiver {
    fn drop(&mut self) {
        if !self.completed {
            // The operation must reach the end of the chain.
            // If it doesn't, then the operation didn't do all it promised it would do.
            // And that means the programmer's expectations no longer hold.
            //
            // We can't communicate the error back either.
            // So panicing is kinda the safest thing to do.
            panic!("start_detached operation did not complete with a signal")
        }
    }
}

#[cfg(test)]
mod test {
    use super::StartDetached;
    use crate::errors::{new_error, ErrorForTesting, Result};
    use crate::just::Just;
    use crate::scheduler::{ImmediateScheduler, Scheduler};
    use crate::then::Then;
    use std::sync::mpsc;

    #[test]
    fn handles_value() {
        let (tx, rx) = mpsc::channel();

        (Just::from((String::from("dcba"),))
            | Then::from(|(x,): (String,)| (x.chars().rev().collect::<String>(),))
            | Then::from(move |(x,)| tx.send(x).map_err(new_error)))
        .start_detached();

        assert_eq!(
            String::from("abcd"),
            rx.recv().expect("value callback was invoked")
        );
    }

    #[test]
    fn handles_done() {
        ImmediateScheduler
            .schedule_done::<(i32, i32, i32)>()
            .start_detached();
    }

    #[test]
    #[should_panic]
    fn handles_error() {
        (Just::from((String::from("dcba"),))
            | Then::from(|(x,): (String,)| (x.chars().rev().collect::<String>(),))
            | Then::from(move |(_,)| -> Result<()> {
                Err(new_error(ErrorForTesting::from(
                    "start_detached error signal test",
                )))
            }))
        .start_detached();
    }
}
