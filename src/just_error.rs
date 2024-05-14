use crate::errors::{Error, IsTuple};
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender};
use core::ops::BitOr;
use std::marker::PhantomData;

pub struct JustError<Tuple: IsTuple> {
    phantom: PhantomData<Tuple>,
    error: Error,
}

impl<Tuple: IsTuple> JustError<Tuple> {
    pub fn new(error: Error) -> JustError<Tuple> {
        JustError {
            error,
            phantom: PhantomData,
        }
    }
}

impl<Tuple: IsTuple> TypedSender for JustError<Tuple> {
    type Value = Tuple;

    fn connect<ReceiverType>(self, receiver: ReceiverType) -> impl OperationState
    where
        ReceiverType: ReceiverOf<Tuple>,
    {
        JustErrorOperationState {
            phantom: PhantomData,
            error: self.error,
            receiver,
        }
    }
}

pub struct JustErrorOperationState<Tuple: IsTuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<Tuple>,
{
    phantom: PhantomData<Tuple>,
    error: Error,
    receiver: ReceiverImpl,
}

impl<Tuple: IsTuple, ReceiverImpl> OperationState for JustErrorOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<Tuple>,
{
    fn start(self) {
        self.receiver.set_error(self.error)
    }
}

impl<Tuple: IsTuple, BindSenderImpl> BitOr<BindSenderImpl> for JustError<Tuple>
where
    BindSenderImpl: BindSender<JustError<Tuple>>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::JustError;
    use crate::sync_wait::sync_wait;
    use std::any::TypeId;

    #[test]
    fn it_works() {
        match sync_wait(JustError::<()>::new(Box::new(String::from("error")))) {
            Ok(_) => panic!("expected an error"),
            Err(e) => {
                assert_eq!(TypeId::of::<String>(), (&*e).type_id());
                assert_eq!(String::from("error"), *e.downcast_ref::<String>().unwrap());
            }
        }
    }
}
