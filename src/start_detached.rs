use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::scope::{detached_scope, ScopeDataSendPtr, ScopeImpl};
use crate::traits::{OperationState, Receiver, ReceiverOf, TypedSenderConnect};
use crate::tuple::Tuple;

/// Start an operation, but don't wait for its completion.
///
/// If the operation completes with an error signal, the code will panic.
/// Otherwise (value or done signal), the code will complete normally.
pub fn start_detached<SenderImpl>(sender: SenderImpl)
where
    SenderImpl: TypedSenderConnect<'static, ScopeImpl<ScopeDataSendPtr>, DiscardingReceiver>,
{
    detached_scope(move |scope: &ScopeImpl<ScopeDataSendPtr>| {
        sender.connect(scope, DiscardingReceiver).start();
    })
}

pub struct DiscardingReceiver;

impl Receiver for DiscardingReceiver {
    fn set_done(self) {}

    fn set_error(self, error: Error) {
        panic!("detached completion failed with error: {:?}", error);
    }
}

impl<Sch, Tpl> ReceiverOf<Sch, Tpl> for DiscardingReceiver
where
    Sch: Scheduler,
    Tpl: Tuple,
{
    fn set_value(self, _: Sch, _: Tpl) {
        // Since we run detached, we discard the arguments.
    }
}

#[cfg(test)]
mod test {
    use super::start_detached;
    use crate::errors::{new_error, ErrorForTesting, Result};
    use crate::just::Just;
    use crate::scheduler::{ImmediateScheduler, Scheduler};
    use crate::then::Then;
    use std::sync::mpsc;

    #[test]
    fn handles_value() {
        let (tx, rx) = mpsc::channel();

        start_detached(
            Just::from((String::from("dcba"),))
                | Then::from(|(x,): (String,)| (x.chars().rev().collect::<String>(),))
                | Then::from(move |(x,)| tx.send(x).map_err(new_error)),
        );

        assert_eq!(
            String::from("abcd"),
            rx.recv().expect("value callback was invoked")
        );
    }

    #[test]
    fn handles_done() {
        start_detached(ImmediateScheduler::default().schedule_done::<(i32, i32, i32)>());
    }

    #[test]
    #[should_panic]
    fn handles_error() {
        start_detached(
            Just::from((String::from("dcba"),))
                | Then::from(|(x,): (String,)| (x.chars().rev().collect::<String>(),))
                | Then::from(move |(_,)| -> Result<()> {
                    Err(new_error(ErrorForTesting::from(
                        "start_detached error signal test",
                    )))
                }),
        );
    }
}
