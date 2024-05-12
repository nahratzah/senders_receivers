use crate::errors::Error;
use crate::traits::{BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender};
use core::ops::BitOr;
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
}

impl<FnType, ArgTuple, Out> Sender for Then<FnType, ArgTuple, Out> where
    FnType: FnOnce(ArgTuple) -> Out
{
}

impl<FnType, Out, NestedSender> BindSender<NestedSender>
    for Then<FnType, <NestedSender as TypedSender>::Value, Out>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Out,
{
    type Output = ThenSender<NestedSender, FnType, Out>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        ThenSender {
            nested,
            fn_impl: self.fn_impl,
            phantom2: PhantomData,
        }
    }
}

impl<NestedSender, FnType, Out, BindSenderImpl> BitOr<BindSenderImpl>
    for ThenSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Out,
    BindSenderImpl: BindSender<ThenSender<NestedSender, FnType, Out>>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ThenSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Out,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom2: PhantomData<Out>,
}

impl<NestedSender, FnType, Out> TypedSender for ThenSender<NestedSender, FnType, Out>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Out,
{
    type Value = Out;

    fn connect<ReceiverImpl>(self, receiver: ReceiverImpl) -> impl OperationState
    where
        ReceiverImpl: ReceiverOf<Out>,
    {
        let wrapped_receiver = ThenWrappedReceiver {
            nested: receiver,
            fn_impl: self.fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
        };

        self.nested.connect(wrapped_receiver)
    }
}

struct ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    nested: ReceiverImpl,
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
}

impl<ReceiverImpl, FnType, ArgTuple, Out> Receiver
    for ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    fn set_done(self) {
        self.nested.set_done()
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error)
    }
}

impl<ReceiverImpl, FnType, ArgTuple, Out> ReceiverOf<ArgTuple>
    for ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: FnOnce(ArgTuple) -> Out,
{
    fn set_value(self, values: ArgTuple) {
        let fn_impl = self.fn_impl;
        let fn_result: Out = fn_impl(values);
        // XXX handle result, iff Out is a result.
        self.nested.set_value(fn_result);
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
            Some((6, 7, 8)),
            sync_wait(Just::new((4, 5, 6)) | Then::new(|(x, y, z)| (x + 2, y + 2, z + 2)))
                .expect("should succeed")
        )
    }
}
