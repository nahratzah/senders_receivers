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
    /// use senders_receivers::io::Write;
    /// use senders_receivers::{SyncWait, ImmediateScheduler, Just, LetValue, Then, Scheduler};
    /// use std::fs::File;
    ///
    /// fn do_the_thing<'a>(file: &'a mut File) {
    ///     let wlen = (
    ///         Just::from((file,))
    ///             | LetValue::from(|sch, (file,): (&mut &'a mut File,)| file.write(sch, b"abcd"))
    ///             | Then::from(|(file, wlen)| (wlen,))
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

pub struct WriteTS<'a, Sch, Fd>
where
    Fd: io::Write + ?Sized,
    Sch: Scheduler,
    Sch::Sender: TypedSender<'a, Value = ()>,
{
    fd: &'a mut Fd,
    buf: &'a [u8],
    sch: Sch,
}

impl<'a, Sch, Fd> TypedSender<'a> for WriteTS<'a, Sch, Fd>
where
    Fd: 'a + io::Write + ?Sized,
    Sch: Scheduler,
    Sch::Sender: TypedSender<'a, Value = ()>,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = (usize,);
}

impl<'scope, 'a, ScopeImpl, ReceiverType, Sch, Fd>
    TypedSenderConnect<'scope, 'a, ScopeImpl, ReceiverType> for WriteTS<'a, Sch, Fd>
where
    'a: 'scope,
    Fd: 'a + io::Write + ?Sized,
    Sch: Scheduler + EnableDefaultIO,
    Sch::Sender: TypedSender<'a, Value = ()>
        + TypedSenderConnect<
            'scope,
            'a,
            ScopeImpl,
            ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Fd>,
        >,
    ReceiverType: 'scope + ReceiverOf<Sch::LocalScheduler, (usize,)>,
    ScopeImpl:
        ScopeWrap<Sch::LocalScheduler, ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Fd>>,
{
    fn connect(self, scope: &ScopeImpl, receiver: ReceiverType) -> impl OperationState<'scope> {
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
    Sch::Sender: TypedSender<'a, Value = ()>,
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
