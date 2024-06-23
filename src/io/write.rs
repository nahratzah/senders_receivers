use crate::errors::{new_error, Error};
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use std::io;
use std::marker::PhantomData;
use std::ops::BitOr;

/// Implement the write trait for a [writeable](io::Write) type.
pub trait Write<Sch>
where
    Sch: Scheduler,
    Self: io::Write,
{
    /// Create a new sender-chain, which will write to this descriptor.
    ///
    /// ```
    /// use senders_receivers::io::Write;
    /// use senders_receivers::{sync_wait, ImmediateScheduler, Just, LetValue, Scheduler};
    /// use std::fs::File;
    ///
    /// fn do_the_thing<'a>(file: &'a mut File) {
    ///     let wlen = sync_wait(
    ///         Just::from((file,))
    ///             | LetValue::from(|sch, (file,): (&'a mut File,)| file.write(sch, b"abcd")),
    ///     )
    ///     .unwrap()
    ///     .unwrap()
    ///     .0;
    ///     println!("wrote {} bytes", wlen);
    /// }
    /// ```
    fn write<'a>(&'a mut self, sch: Sch, buf: &'a [u8]) -> WriteTS<'a, Sch::Sender, Self>;
}

impl<Sch, T> Write<Sch> for T
where
    Sch: Scheduler + 'static,
    T: io::Write,
{
    fn write<'a>(&'a mut self, sch: Sch, buf: &'a [u8]) -> WriteTS<'a, Sch::Sender, Self> {
        WriteTS {
            fd: self,
            buf,
            nested_sender: sch.schedule(),
        }
    }
}

pub struct WriteTS<'a, NestedSender, Fd>
where
    Fd: io::Write + ?Sized,
    NestedSender: TypedSender<'a, Value = ()>,
{
    fd: &'a mut Fd,
    buf: &'a [u8],
    nested_sender: NestedSender,
}

impl<'a, NestedSender, Fd> TypedSender<'a> for WriteTS<'a, NestedSender, Fd>
where
    Fd: 'a + io::Write + ?Sized,
    NestedSender: TypedSender<'a, Value = ()>,
{
    type Scheduler = NestedSender::Scheduler;
    type Value = (usize,);
}

impl<'a, ReceiverType, NestedSender, Fd> TypedSenderConnect<'a, ReceiverType>
    for WriteTS<'a, NestedSender, Fd>
where
    Fd: 'a + io::Write + ?Sized,
    NestedSender: TypedSender<'a, Value = ()>
        + TypedSenderConnect<
            'a,
            ReceiverWrapper<'a, ReceiverType, <NestedSender as TypedSender<'a>>::Scheduler, Fd>,
        >,
    ReceiverType: ReceiverOf<<NestedSender as TypedSender<'a>>::Scheduler, (usize,)>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        self.nested_sender.connect(ReceiverWrapper {
            nested: receiver,
            fd: self.fd,
            buf: self.buf,
            phantom: PhantomData,
        })
    }
}

impl<'a, BindSenderImpl, NestedSender, Fd> BitOr<BindSenderImpl> for WriteTS<'a, NestedSender, Fd>
where
    BindSenderImpl: BindSender<Self>,
    NestedSender: TypedSender<'a, Value = ()>,
    Fd: 'a + io::Write + ?Sized,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<'a, ReceiverType, Sch, Fd>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: 'a + io::Write + ?Sized,
{
    phantom: PhantomData<Sch>,
    nested: ReceiverType,
    fd: &'a mut Fd,
    buf: &'a [u8],
}

impl<'a, ReceiverType, Sch, Fd> Receiver for ReceiverWrapper<'a, ReceiverType, Sch, Fd>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: 'a + io::Write + ?Sized,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<'a, Sch, ReceiverType, Fd> ReceiverOf<Sch, ()> for ReceiverWrapper<'a, ReceiverType, Sch, Fd>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: 'a + io::Write + ?Sized,
{
    fn set_value(self, sch: Sch, _: ()) {
        match self.fd.write(self.buf) {
            Ok(len) => self.nested.set_value(sch, (len,)),
            Err(error) => self.nested.set_error(new_error(error)),
        };
    }
}
