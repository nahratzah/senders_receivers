use crate::errors::Error;
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use crate::tuple::Tuple;
use std::marker::PhantomData;
use std::ops::BitOr;

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
pub struct JustError<Sch: Scheduler, Tpl: Tuple> {
    phantom: PhantomData<fn(Sch) -> Tpl>,
    error: Error,
}

impl<Sch: Scheduler, Tpl: Tuple> JustError<Sch, Tpl> {
    /// Create a new typed sender that'll yield an error.
    ///
    /// This function usually requires a turbo-fish to get the type right.
    /// So you might prefer to use [Scheduler::schedule_error] instead.
    pub fn new(error: Error) -> Self {
        JustError {
            error,
            phantom: PhantomData,
        }
    }
}

impl<Tpl: Tuple> From<Error> for JustError<ImmediateScheduler, Tpl> {
    /// Create a new typed sender that'll yield an error.
    fn from(error: Error) -> Self {
        Self::new(error)
    }
}

impl<Sch: Scheduler, Tpl: Tuple> WithScheduler<Sch, Error> for JustError<Sch, Tpl> {
    fn with_scheduler(_: Sch, error: Error) -> Self {
        JustError {
            error,
            phantom: PhantomData,
        }
    }
}

impl<Sch: Scheduler, Tpl: Tuple> TypedSender<'_> for JustError<Sch, Tpl> {
    type Value = Tpl;
    type Scheduler = Sch::LocalScheduler;
}

impl<'scope, 'a, ScopeImpl, ReceiverType, Sch, Tpl>
    TypedSenderConnect<'scope, 'a, ScopeImpl, ReceiverType> for JustError<Sch, Tpl>
where
    'a: 'scope,
    Sch: Scheduler,
    Tpl: Tuple,
    ReceiverType: 'scope + ReceiverOf<Sch::LocalScheduler, Tpl> + 'a,
{
    fn connect(self, _: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope> {
        JustErrorOperationState {
            error: self.error,
            receiver,
            phantom: PhantomData,
        }
    }
}

pub struct JustErrorOperationState<'a, ReceiverImpl>
where
    ReceiverImpl: Receiver,
{
    phantom: PhantomData<&'a i32>,
    error: Error,
    receiver: ReceiverImpl,
}

impl<'a, ReceiverImpl> OperationState<'a> for JustErrorOperationState<'a, ReceiverImpl>
where
    ReceiverImpl: Receiver,
{
    fn start(self) {
        self.receiver.set_error(self.error);
    }
}

impl<Sch: Scheduler, Tpl: Tuple, BindSenderImpl> BitOr<BindSenderImpl> for JustError<Sch, Tpl>
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
