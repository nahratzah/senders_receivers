use crate::traits::{OperationState, ReceiverOf, ReceiverOfError, TypedSender};

pub struct Just<Tuple> {
    values: Tuple,
}

impl<Tuple> Just<Tuple> {
    pub fn new(init: Tuple) -> Just<Tuple> {
        Just::<Tuple> { values: init }
    }
}

impl<Tuple> TypedSender<Tuple, ()> for Just<Tuple> {
    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Tuple> + ReceiverOfError<()>,
    {
        JustOperationState {
            values: self.values,
            receiver,
        }
    }
}

pub struct JustOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<Tuple> + ReceiverOfError<()>,
{
    values: Tuple,
    receiver: ReceiverImpl,
}

impl<Tuple, ReceiverImpl> OperationState for JustOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<Tuple> + ReceiverOfError<()>,
{
    fn start(self) {
        self.receiver.set_value(self.values)
    }
}

#[cfg(test)]
mod tests {
    use super::Just;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(Ok(Some((4, 5, 6))), sync_wait(Just::new((4, 5, 6))))
    }
}
