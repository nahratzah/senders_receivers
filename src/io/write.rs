use crate::errors::{new_error, Error};
use crate::io::default::EnableDefaultIO;
use crate::scheduler::Scheduler;
use crate::scope::ScopeWrap;
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
    /// use senders_receivers::io::{Write, WriteTS};
    /// use senders_receivers::{SyncWait, ImmediateScheduler, Just, LetValue, Then, Scheduler, TypedSender, Result};
    /// use senders_receivers::tuple::*; // for distribute.
    /// use std::fs::File;
    ///
    /// fn do_the_thing<'a>(file: &'a mut File) {
    ///     let wlen = (
    ///         Just::from((file, String::from("abcd")))
    ///             | LetValue::from(|sch, v: &mut (&'a mut File, String)| v.0.write(sch, v.1.as_bytes()))
    ///             | Then::from(|(_, _, wlen)| (wlen,))
    ///         )
    ///         .sync_wait()
    ///         .unwrap()
    ///         .unwrap()
    ///         .0;
    ///     println!("wrote {} bytes", wlen);
    /// }
    /// ```
    fn write<'a>(&'a mut self, sch: Sch, buf: &'a [u8]) -> WriteTS<'a, Sch, Self>;
}

impl<Sch, T> Write<Sch> for T
where
    Sch: Scheduler,
    T: io::Write,
{
    fn write<'a>(&'a mut self, sch: Sch, buf: &'a [u8]) -> WriteTS<'a, Sch, Self> {
        WriteTS { fd: self, buf, sch }
    }
}

/// Typed-sender returned by the [Write] trait.
pub struct WriteTS<'a, Sch, Fd>
where
    Fd: io::Write + ?Sized,
    Sch: Scheduler,
    Sch::Sender: TypedSender<Value = ()>,
{
    fd: &'a mut Fd,
    buf: &'a [u8],
    sch: Sch,
}

impl<'a, Sch, Fd> TypedSender for WriteTS<'a, Sch, Fd>
where
    Fd: 'a + io::Write + ?Sized,
    Sch: Scheduler,
    Sch::Sender: TypedSender<Value = ()>,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = (usize,);
}

impl<'a, ScopeImpl, ReceiverType, Sch, Fd> TypedSenderConnect<'a, ScopeImpl, ReceiverType>
    for WriteTS<'a, Sch, Fd>
where
    Fd: 'a + io::Write + ?Sized,
    Sch: Scheduler + EnableDefaultIO,
    Sch::Sender: TypedSender<Value = ()>
        + TypedSenderConnect<
            'a,
            ScopeImpl,
            ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Fd>,
        >,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, (usize,)>,
    ScopeImpl:
        ScopeWrap<Sch::LocalScheduler, ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Fd>>,
{
    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        receiver: ReceiverType,
    ) -> impl OperationState<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        ReceiverType: 'scope,
    {
        self.sch.schedule().connect(
            scope,
            ReceiverWrapper {
                nested: receiver,
                fd: self.fd,
                buf: self.buf,
                phantom: PhantomData,
            },
        )
    }
}

impl<'a, BindSenderImpl, Sch, Fd> BitOr<BindSenderImpl> for WriteTS<'a, Sch, Fd>
where
    BindSenderImpl: BindSender<Self>,
    Sch: Scheduler,
    Sch::Sender: TypedSender<Value = ()>,
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
