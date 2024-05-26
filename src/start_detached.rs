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

impl<Sch, Value> ReceiverOf<Sch, Value> for DiscardingReceiver
where
    Sch: Scheduler,
    Value: IsTuple,
{
    fn set_value(self, _: Sch, _: Value) {
        // Since we run detached, we discard the arguments.
    }
}

#[cfg(test)]
mod test {
    use super::start_detached;
    use crate::errors::{new_error, Error, ErrorForTesting};
    use crate::just::Just;
    use crate::then::Then;
    use std::sync::mpsc;

    #[test]
    fn handles_value() {
        let (tx, rx) = mpsc::channel();

        start_detached(
            Just::new((String::from("dcba"),))
                | Then::from(|(x,): (String,)| (x.chars().rev().collect::<String>(),))
                | Then::from(move |(x,)| tx.send(x).map_err(|e| new_error(e))),
        );

        assert_eq!(
            String::from("abcd"),
            rx.recv().expect("value callback was invoked")
        );
    }

    // #[test]
    // fn handles_done() {
    //     let (tx, rx) = mpsc::channel();
    //
    //     start_detached(
    //         Just::new((String::from("dcba"),))
    //         | LetValue::new_fn(|_, (_,): (String,)| JustDone::<(String,)>::default())
    //         | Then::from(move |(x,)| -> Result<(), Error> { // XXX change to something that handle 'done'
    //     	tx.send(x).map_err(|e| new_error(e)) // Never runs.
    //     	}));
    //
    //     assert_eq!(
    //         String::from("abcd"),
    //         rx.recv().expect("done callback was invoked"));
    // }

    #[test]
    #[should_panic]
    fn handles_error() {
        start_detached(
            Just::new((String::from("dcba"),))
                | Then::from(|(x,): (String,)| (x.chars().rev().collect::<String>(),))
                | Then::from(move |(_,)| -> Result<(), Error> {
                    Err(new_error(ErrorForTesting::from(
                        "start_detached error signal test",
                    )))
                }),
        );
    }
}
