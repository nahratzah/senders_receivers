use crate::traits::{OperationState, Receiver, ReceiverOf, ReceiverOfError, Sender, TypedSender};
use std::marker::PhantomData;

pub struct Then<FnType, ArgTuple, Out>
where
    FnType: FnOnce(ArgTuple) -> Out,
{
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<FnType, ArgTuple, Out> Then<FnType, ArgTuple, Out>
where
    FnType: FnOnce(ArgTuple) -> Out,
{
    pub fn new(fn_impl: FnType) -> Then<FnType, ArgTuple, Out> {
        Then {
            fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
        }
    }

    pub fn bind<Error>(
        self,
        nested: impl TypedSender<ArgTuple, Error>,
    ) -> impl TypedSender<Out, Error> {
        ThenSender {
            nested,
            fn_impl: self.fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
            phantom3: PhantomData,
        }
    }
}

impl<FnType, ArgTuple, Out> Sender for Then<FnType, ArgTuple, Out> where
    FnType: FnOnce(ArgTuple) -> Out
{
}

struct ThenSender<NestedSender, FnType, ArgTuple, Out, Error>
where
    NestedSender: TypedSender<ArgTuple, Error>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
    phantom3: PhantomData<Error>,
}

impl<NestedSender, FnType, ArgTuple, Out, Error> TypedSender<Out, Error>
    for ThenSender<NestedSender, FnType, ArgTuple, Out, Error>
where
    NestedSender: TypedSender<ArgTuple, Error>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    fn connect<ReceiverImpl>(self, receiver: ReceiverImpl) -> impl OperationState
    where
        ReceiverImpl: ReceiverOf<Out> + ReceiverOfError<Error>,
    {
        let wrapped_receiver = ThenWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
            phantom3: PhantomData,
        };

        self.nested.connect(wrapped_receiver)
    }
}

struct ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out, Error>
where
    ReceiverImpl: ReceiverOf<Out> + ReceiverOfError<Error>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
    phantom3: PhantomData<Error>,
}

impl<ReceiverImpl, FnType, ArgTuple, Out, Error> Receiver
    for ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out, Error>
where
    ReceiverImpl: ReceiverOf<Out> + ReceiverOfError<Error>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    fn set_done(self) {
        self.nested.set_done()
    }
}

impl<ReceiverImpl, FnType, ArgTuple, Out, Error> ReceiverOf<ArgTuple>
    for ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out, Error>
where
    ReceiverImpl: ReceiverOf<Out> + ReceiverOfError<Error>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    fn set_value(self, values: ArgTuple) {
        let fn_impl = self.fn_impl;
        let fn_result: Out = fn_impl(values);
        // XXX handle result, iff Out is a result.
        self.nested.set_value(fn_result);
    }
}

impl<ReceiverImpl, FnType, ArgTuple, Out, Error> ReceiverOfError<Error>
    for ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out, Error>
where
    ReceiverImpl: ReceiverOf<Out> + ReceiverOfError<Error>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    fn set_error(self, error: Error) {
        self.nested.set_error(error)
    }
}

#[cfg(test)]
mod tests {
    use super::Then;
    use crate::just::Just;
    use crate::sync_wait::sync_wait;

    #[test]
    fn it_works() {
        assert_eq!(
            Ok(Some((6, 7, 8))),
            sync_wait(Then::new(|(x, y, z)| (x + 2, y + 2, z + 2)).bind(Just::new((4, 5, 6))))
        )
    }
}
