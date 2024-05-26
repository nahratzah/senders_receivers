use crate::errors::{Error, IsTuple};
use crate::scheduler::{Scheduler, WithScheduler};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use core::ops::BitOr;
use std::marker::PhantomData;

/// A [TypedSender] that always generates an [Error].
///
/// Example:
/// ```
/// use senders_receivers::{ImmediateScheduler, JustError, new_error, sync_wait};
///
/// fn example(someError: impl std::error::Error + Send + 'static) {
///     // The `::<(i32, i32)>` turbo-fish is to declare the value-type of the created sender.
///     let sender = JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(someError));
///     match sync_wait(sender) {
///         Ok(_) => panic!("there won't be a value"),
///         Err(e) => println!("{:?}", e),  // `someError` ends up in `e`
///     };
/// }
/// ```
pub struct JustError<Sch: Scheduler, Tuple: IsTuple> {
    phantom: PhantomData<fn(Sch) -> Tuple>,
    error: Error,
}

impl<Sch: Scheduler, Tuple: IsTuple> From<Error> for JustError<Sch, Tuple> {
    /// Create a new typed sender that'll yield an error.
    fn from(error: Error) -> Self {
        JustError {
            error,
            phantom: PhantomData,
        }
    }
}

impl<Sch: Scheduler, Tuple: IsTuple> WithScheduler<Sch, Error> for JustError<Sch, Tuple> {
    fn with_scheduler(_: Sch, error: Error) -> Self {
        JustError {
            error,
            phantom: PhantomData,
        }
    }
}

impl<Sch: Scheduler, Tuple: IsTuple> TypedSender for JustError<Sch, Tuple> {
    type Value = Tuple;
    type Scheduler = Sch::LocalScheduler;
}

impl<ReceiverType, Sch, Tuple> TypedSenderConnect<ReceiverType> for JustError<Sch, Tuple>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Tuple: IsTuple,
    ReceiverType: ReceiverOf<Sch, Tuple>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        JustErrorOperationState {
            error: self.error,
            receiver,
        }
    }
}

pub struct JustErrorOperationState<ReceiverImpl>
where
    ReceiverImpl: Receiver,
{
    error: Error,
    receiver: ReceiverImpl,
}

impl<ReceiverImpl> OperationState for JustErrorOperationState<ReceiverImpl>
where
    ReceiverImpl: Receiver,
{
    fn start(self) {
        self.receiver.set_error(self.error);
    }
}

impl<Sch: Scheduler, Tuple: IsTuple, BindSenderImpl> BitOr<BindSenderImpl> for JustError<Sch, Tuple>
where
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

#[cfg(test)]
mod tests {
    use super::JustError;
    use crate::errors::{new_error, ErrorForTesting};
    use crate::scheduler::ImmediateScheduler;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        match sync_wait(JustError::<ImmediateScheduler, ()>::from(new_error(
            ErrorForTesting::from("error"),
        ))) {
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
