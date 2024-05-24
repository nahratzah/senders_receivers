use crate::errors::{Error, IsTuple};
use crate::scheduler::ImmediateScheduler;
use crate::traits::{BindSender, OperationState, ReceiverOf, TypedSender, TypedSenderConnect};
use core::ops::BitOr;
use std::marker::PhantomData;

/// A [TypedSender] that always generates an [Error].
///
/// Example:
/// ```
/// use senders_receivers::{JustError, sync_wait};
///
/// fn example(someError: impl std::error::Error + Send + 'static) {
///     // The `::<(i32, i32)>` turbo-fish is to declare the value-type of the created sender.
///     let sender = JustError::<(i32, i32)>::new(Box::new(someError));
///     match sync_wait(sender) {
///         Ok(_) => panic!("there won't be a value"),
///         Err(e) => println!("{:?}", e),  // `someError` ends up in `e`
///     };
/// }
/// ```
pub struct JustError<Tuple: IsTuple> {
    phantom: PhantomData<fn() -> Tuple>,
    error: Error,
}

impl<Tuple: IsTuple> JustError<Tuple> {
    /// Create a new typed sender that'll yield an error.
    pub fn new(error: Error) -> JustError<Tuple> {
        JustError {
            error,
            phantom: PhantomData,
        }
    }
}

impl<Tuple: IsTuple> TypedSender for JustError<Tuple> {
    type Value = Tuple;
    type Scheduler = ImmediateScheduler;
}

impl<ReceiverType, Tuple> TypedSenderConnect<ReceiverType> for JustError<Tuple>
where
    Tuple: IsTuple,
    ReceiverType: ReceiverOf<ImmediateScheduler, Tuple>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        JustErrorOperationState {
            phantom: PhantomData,
            error: self.error,
            receiver,
        }
    }
}

pub struct JustErrorOperationState<Tuple: IsTuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<ImmediateScheduler, Tuple>,
{
    phantom: PhantomData<fn() -> Tuple>,
    error: Error,
    receiver: ReceiverImpl,
}

impl<Tuple: IsTuple, ReceiverImpl> OperationState for JustErrorOperationState<Tuple, ReceiverImpl>
where
    ReceiverImpl: ReceiverOf<ImmediateScheduler, Tuple>,
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
    use crate::errors::ErrorForTesting;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        match sync_wait(JustError::<()>::new(Box::new(ErrorForTesting::from(
            "error",
        )))) {
            Ok(_) => panic!("expected an error"),
            Err(e) => {
                //assert_eq!(TypeId::of::<ErrorForTesting>(), (&*e).type_id());
                assert_eq!(
                    ErrorForTesting::from("error"),
                    *e.downcast_ref::<ErrorForTesting>().unwrap()
                );
            }
        }
    }
}
