use crate::errors::{new_error, Error};
use crate::scheduler::Scheduler;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};

use std::io;


use std::ops::BitOr;

pub trait Write<Sch>
where
    Sch: Scheduler,
    Self: io::Write,
{
    /// ---
    /// use crate::io::Write;
    /// use crate::{sync_wait, ImmediateScheduler, Just, LetValue, Scheduler};
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
        WriteSender::<'a> { sch, fd: self, buf }
    }
}

struct WriteSender<'a, Sch, Fd>
where
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
{
    sch: Sch,
    fd: &'a mut Fd,
    buf: &'a [u8],
}

impl<'a, Sch, Fd> TypedSender for WriteSender<'a, Sch, Fd>
where
    Sch: Scheduler,
    Fd: 'a + io::Write + ?Sized,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = (usize,);
}

impl<'a, ReceiverType, Sch, Fd> TypedSenderConnect<ReceiverType> for WriteSender<'a, Sch, Fd>
where
    Sch: Scheduler,
    Fd: 'a + io::Write + ?Sized,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, (usize,)>,
    Sch::Sender: TypedSenderConnect<ReceiverWrapper<'a, ReceiverType, Fd>>,
{
    fn connect(self, receiver: ReceiverType) -> impl OperationState {
        self.sch.schedule().connect(ReceiverWrapper {
            nested: receiver,
            fd: self.fd,
            buf: self.buf,
        })
    }
}

impl<'a, BindSenderImpl, Sch, Fd> BitOr<BindSenderImpl> for WriteSender<'a, Sch, Fd>
where
    BindSenderImpl: BindSender<Self>,
    Sch: Scheduler,
    Fd: 'a + io::Write + ?Sized,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<'a, ReceiverType, Fd>
where
    ReceiverType: Receiver,
    Fd: 'a + io::Write + ?Sized,
{
    nested: ReceiverType,
    fd: &'a mut Fd,
    buf: &'a [u8],
}

impl<'a, ReceiverType, Fd> Receiver for ReceiverWrapper<'a, ReceiverType, Fd>
where
    ReceiverType: Receiver,
    Fd: 'a + io::Write + ?Sized,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<'a, Sch, ReceiverType, Fd> ReceiverOf<Sch, ()> for ReceiverWrapper<'a, ReceiverType, Fd>
where
    Sch: Scheduler,
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Fd: 'a + io::Write + ?Sized,
{
    fn set_value(self, sch: Sch, _: ()) {
        match self.fd.write(self.buf) {
            Ok(len) => self.nested.set_value(sch, (len,)),
            Err(error) => self.nested.set_error(new_error(error)),
        };
    }
}
