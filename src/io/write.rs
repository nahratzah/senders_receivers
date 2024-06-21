use crate::errors::{new_error, Error};
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect,
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
    /// ---
    /// use senders_receivers::io::Write;
    /// use senders_receivers::{sync_wait, ImmediateScheduler, Just, LetValue, Scheduler};
    /// use std::fs::File;
    ///
    /// fn do_the_thing(file: &mut File) {
    ///     let wlen = sync_wait(
    ///         Just::from(())
    ///             | LetValue::from(|sch: ImmediateScheduler, _| file.write(sch.lazy(), b"abcd")),
    ///     )
    ///     .unwrap()
    ///     .unwrap()
    ///     .0;
    ///     println!("wrote {} bytes", wlen);
    /// }
    /// ---
    fn write<'a>(
        &'a mut self,
        sch: Sch,
        buf: &'a [u8],
    ) -> impl TypedSender<Scheduler = Sch::LocalScheduler, Value = (usize,)> + 'a;
}

impl<Sch, T> Write<Sch> for T
where
    Sch: Scheduler + 'static,
    T: io::Write,
{
    fn write<'a>(
        &'a mut self,
        sch: Sch,
        buf: &'a [u8],
    ) -> impl TypedSender<Scheduler = Sch::LocalScheduler, Value = (usize,)> + 'a {
        WriteSender::<'a> { fd: self, buf }.bind(sch.schedule())
    }
}

struct WriteSender<'a, Fd>
where
    Fd: io::Write + ?Sized,
{
    fd: &'a mut Fd,
    buf: &'a [u8],
}

impl<'a, Fd> Sender for WriteSender<'a, Fd> where Fd: io::Write + ?Sized {}

impl<'a, Fd, NestedSender> BindSender<NestedSender> for WriteSender<'a, Fd>
where
    Fd: io::Write + ?Sized,
    NestedSender: TypedSender<Value = ()>,
{
    type Output = WriteTS<'a, NestedSender, Fd>;

    fn bind(self, nested_sender: NestedSender) -> Self::Output {
        WriteTS {
            nested_sender,
            fd: self.fd,
            buf: self.buf,
        }
    }
}

struct WriteTS<'a, NestedSender, Fd>
where
    Fd: io::Write + ?Sized,
    NestedSender: TypedSender<Value = ()>,
{
    fd: &'a mut Fd,
    buf: &'a [u8],
    nested_sender: NestedSender,
}

impl<'a, NestedSender, Fd> TypedSender for WriteTS<'a, NestedSender, Fd>
where
    Fd: 'a + io::Write + ?Sized,
    NestedSender: TypedSender<Value = ()>,
{
    type Scheduler = NestedSender::Scheduler;
    type Value = (usize,);
}

impl<'a, ReceiverType, NestedSender, Fd> TypedSenderConnect<ReceiverType>
    for WriteTS<'a, NestedSender, Fd>
where
    Fd: 'a + io::Write + ?Sized,
    NestedSender: TypedSender<Value = ()>
        + TypedSenderConnect<
            ReceiverWrapper<'a, ReceiverType, <NestedSender as TypedSender>::Scheduler, Fd>,
        >,
    ReceiverType: ReceiverOf<<NestedSender as TypedSender>::Scheduler, (usize,)>,
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
    NestedSender: TypedSender<Value = ()>,
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
