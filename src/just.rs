use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender};
use core::ops::BitOr;

pub struct Just<Tuple> {
    values: Tuple,
}

impl<Tuple> Just<Tuple> {
    pub fn new(init: Tuple) -> Just<Tuple> {
        Just::<Tuple> { values: init }
    }
}

impl<Tuple> TypedSender for Just<Tuple> {
    type Value = Tuple;

    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Tuple>,
    {
        JustOperationState {
            values: self.values,
            receiver,
        }
    }
}

pub struct JustOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<Tuple>,
{
    values: Tuple,
    receiver: ReceiverImpl,
}

impl<Tuple, ReceiverImpl> OperationState for JustOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<Tuple>,
{
    fn start(self) {
        self.receiver.set_value(self.values)
    }
}

impl<Tuple, BindSenderImpl> BitOr<BindSenderImpl> for Just<Tuple>
where
    BindSenderImpl: BindSender<Just<Tuple>>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Just;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(
            Some((4, 5, 6)),
            sync_wait(Just::new((4, 5, 6))).expect("just() should not fail")
        )
    }
}
