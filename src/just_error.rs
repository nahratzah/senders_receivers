use crate::errors::Error;
use crate::scheduler::{ImmediateScheduler, Scheduler, WithScheduler};
use crate::stop_token::StopToken;
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
/// use senders_receivers::{ImmediateScheduler, JustError, new_error, SyncWait};
///
/// fn example(someError: impl std::error::Error + Send + Sync + 'static) {
///     // The `::<(i32, i32)>` turbo-fish is to declare the value-type of the created sender.
///     let sender = JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(someError));
///     match sender.sync_wait() {
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

impl<Sch: Scheduler, Tpl: Tuple> TypedSender for JustError<Sch, Tpl> {
    type Value = Tpl;
    type Scheduler = Sch::LocalScheduler;
}

impl<'a, ScopeImpl, StopTokenImpl, ReceiverType, Sch, Tpl>
    TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverType> for JustError<Sch, Tpl>
where
    Sch: Scheduler,
    Tpl: Tuple,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, Tpl> + 'a,
    StopTokenImpl: StopToken,
{
    type Output<'scope> = JustErrorOperationState<'scope, ReceiverType>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope;

    fn connect<'scope>(
        self,
        _: &ScopeImpl,
        _: StopTokenImpl,
        receiver: ReceiverType,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        StopTokenImpl: 'scope,
        ReceiverType: 'scope,
    {
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
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_works() {
        match (JustError::<ImmediateScheduler, ()>::from(new_error(ErrorForTesting::from("error"))))
            .sync_wait()
        {
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
