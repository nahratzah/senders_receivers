use crate::errors::{IsTuple, NoError};
use crate::traits::{BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender};
use core::ops::BitOr;
use std::marker::PhantomData;

pub struct Then<FnType, ArgTuple: IsTuple, Out: IsTuple, Error>
where
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>,
    Error: 'static,
{
    fn_impl: FnType,
    phantom1: PhantomData<ArgTuple>,
    phantom2: PhantomData<Out>,
    phantom3: PhantomData<Error>,
}

impl<FnType, ArgTuple: IsTuple, Out: IsTuple, Error> Then<FnType, ArgTuple, Out, Error>
where
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>,
{
    pub fn new_err(fn_impl: FnType) -> Then<FnType, ArgTuple, Out, Error> {
        Then {
            fn_impl,
            phantom1: PhantomData,
            phantom2: PhantomData,
            phantom3: PhantomData,
        }
    }
}

// When we have a function that doesn't return an error, we wrap it, to return an error anyway.
// (Yeah, it's inefficient, I know.) :/
type BoxedWrappedFnType<ArgTuple, Out> = Box<dyn FnOnce(ArgTuple) -> Result<Out, NoError>>;

impl<ArgTuple: IsTuple, Out: IsTuple>
    Then<Box<dyn FnOnce(ArgTuple) -> Result<Out, NoError>>, ArgTuple, Out, NoError>
{
    pub fn new<NoErrFnType>(
        fn_impl: NoErrFnType,
    ) -> Then<BoxedWrappedFnType<ArgTuple, Out>, ArgTuple, Out, NoError>
    where
        NoErrFnType: 'static + FnOnce(ArgTuple) -> Out,
    {
        Then {
            fn_impl: Box::new(move |arg| Ok(fn_impl(arg))),
            phantom1: PhantomData,
            phantom2: PhantomData,
            phantom3: PhantomData,
        }
    }
}

impl<FnType, ArgTuple: IsTuple, Out: IsTuple, Error> Sender for Then<FnType, ArgTuple, Out, Error> where
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>
{
}

impl<FnType, Out: IsTuple, Error, NestedSender> BindSender<NestedSender>
    for Then<FnType, <NestedSender as TypedSender>::Value, Out, Error>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Result<Out, Error>,
    <NestedSender as TypedSender>::Value: IsTuple,
{
    type Output = ThenSender<NestedSender, FnType, Out, Error>;

    fn bind(self, nested: NestedSender) -> Self::Output {
        ThenSender {
            nested,
            fn_impl: self.fn_impl,
            phantom2: PhantomData,
            phantom3: PhantomData,
        }
    }
}

impl<NestedSender, FnType, Out: IsTuple, Error, BindSenderImpl> BitOr<BindSenderImpl>
    for ThenSender<NestedSender, FnType, Out, Error>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Result<Out, Error>,
    BindSenderImpl: BindSender<ThenSender<NestedSender, FnType, Out, Error>>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

pub struct ThenSender<NestedSender, FnType, Out: IsTuple, Error>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Result<Out, Error>,
    Error: 'static,
{
    nested: NestedSender,
    fn_impl: FnType,
    phantom2: PhantomData<Out>,
    phantom3: PhantomData<Error>,
}

impl<NestedSender, FnType, Out, Error> TypedSender for ThenSender<NestedSender, FnType, Out, Error>
where
    NestedSender: TypedSender,
    FnType: FnOnce(<NestedSender as TypedSender>::Value) -> Result<Out, Error>,
    Out: IsTuple,
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
            phantom3: PhantomData,
        };

        self.nested.connect(wrapped_receiver)
    }
}

struct ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out, Error>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>,
    Out: IsTuple,
    Error: 'static,
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
    ReceiverImpl: ReceiverOf<Out>,
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>,
    Out: IsTuple,
{
    fn set_done(self) {
        self.nested.set_done()
    }

    fn set_error(self, error: crate::errors::Error) {
        self.nested.set_error(error)
    }
}

impl<ReceiverImpl, FnType, ArgTuple, Out, Error> ReceiverOf<ArgTuple>
    for ThenWrappedReceiver<ReceiverImpl, FnType, ArgTuple, Out, Error>
where
    ReceiverImpl: ReceiverOf<Out>,
    FnType: FnOnce(ArgTuple) -> Result<Out, Error>,
    ArgTuple: IsTuple,
    Out: IsTuple,
{
    fn set_value(self, values: ArgTuple) {
        let fn_impl = self.fn_impl;
        match fn_impl(values) {
            Ok(v) => self.nested.set_value(v),
            Err(e) => self.nested.set_error(Box::new(e)),
        }
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

    #[test]
    fn it_works_with_errors() {
        assert_eq!(
            Some((6, 7, 8)),
            sync_wait(
                Just::new((4, 5, 6))
                    | Then::new_err(|(x, y, z)| -> Result<(i32, i32, i32), String> {
                        Ok((x + 2, y + 2, z + 2))
                    })
            )
            .expect("should succeed")
        )
    }
}
