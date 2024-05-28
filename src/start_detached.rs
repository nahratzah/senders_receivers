use crate::errors::{Error, IsTuple};
use crate::scheduler::Scheduler;
use crate::traits::{OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect};

/// Start an operation, but don't wait for its completion.
///
/// If the operation completes with an error signal, the code will panic.
/// Otherwise (value or done signal), the code will complete normally.
pub fn start_detached<SenderImpl>(sender: SenderImpl)
where
    SenderImpl: TypedSender + TypedSenderConnect<DiscardingReceiver>,
{
    sender.connect(DiscardingReceiver).start();
}

pub struct DiscardingReceiver;

impl Receiver for DiscardingReceiver {
    fn set_done(self) {}

    fn set_error(self, error: Error) {
        panic!("detached completion failed with error: {:?}", error);
    }
}

impl<Sch, Tuple> ReceiverOf<Sch, Tuple> for DiscardingReceiver
where
    Sch: Scheduler,
    Tuple: IsTuple,
{
    fn set_value(self, _: Sch, _: Tuple) {
        // Since we run detached, we discard the arguments.
    }
}

#[cfg(test)]
mod test {
    use super::start_detached;
    use crate::errors::{new_error, Error, ErrorForTesting};
    use crate::just::Just;
    use crate::let_value::LetValue;
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
        start_detached(
            Just::from((String::from("dcba"),))
                | LetValue::from(|sch: ImmediateScheduler, _| {
                    sch.schedule_done::<(i32, i32, i32)>()
                }),
        );
    }

    #[test]
    #[should_panic]
    fn handles_error() {
        start_detached(
            Just::from((String::from("dcba"),))
                | Then::from(|(x,): (String,)| (x.chars().rev().collect::<String>(),))
                | Then::from(move |(_,)| -> Result<(), Error> {
                    Err(new_error(ErrorForTesting::from(
                        "start_detached error signal test",
                    )))
                }),
        );
    }
}
