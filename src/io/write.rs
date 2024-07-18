use crate::errors::{new_error, Error};
use crate::io::default::EnableDefaultIO;
use crate::refs;
use crate::scheduler::Scheduler;
use crate::scope::ScopeWrap;
use crate::traits::{
    BindSender, OperationState, Receiver, ReceiverOf, TypedSender, TypedSenderConnect,
};
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::ops::BitOr;
use std::ops::DerefMut;

/// Implement the write trait for a [writeable](io::Write) type.
pub trait Write<Sch, SelfState, BufState>
where
    Sch: Scheduler,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
    Self: DerefMut,
    Self::Target: 'static + io::Write,
{
    /// Create a new sender-chain, which will write to this descriptor.
    ///
    /// ```
    /// use senders_receivers::io::Write;
    /// use senders_receivers::{SyncWait, Just, LetValue, Then, new_error};
    /// use senders_receivers::tuple::*; // for distribute.
    /// use senders_receivers::refs;
    /// use std::fs::File;
    ///
    /// fn create_file(filename: String, buf: String) {
    ///     let (wlen,) = (
    ///         Just::from((filename, buf))
    ///         | Then::from(|(filename, buf)| {
    ///               match File::create(filename) {
    ///                   Ok(file) => Ok((file, buf)),
    ///                   Err(error) => Err(new_error(error)),
    ///               }
    ///           })
    ///         | LetValue::from(|sch, v: refs::ScopedRefMut<(File, String), refs::NoSendState>| {
    ///               let (file, buf) = DistributeRefTuple::distribute(v);
    ///               file.write(sch, refs::ScopedRefMut::map_no_mut(buf, |x| x.as_bytes()))
    ///           })
    ///         | Then::from(|(_, _, wlen)| (wlen,))
    ///     )
    ///         .sync_wait()
    ///         .unwrap()
    ///         .unwrap();
    ///     println!("wrote {} bytes", wlen);
    /// }
    /// ```
    fn write(
        self,
        sch: Sch,
        buf: refs::ScopedRef<[u8], BufState>,
    ) -> WriteTS<Sch, Self::Target, SelfState, BufState>;

    /// Create a new sender-chain, which will write to this descriptor.
    ///
    /// ```
    /// use senders_receivers::io::Write;
    /// use senders_receivers::{SyncWait, Just, LetValue, Then, new_error};
    /// use senders_receivers::tuple::*; // for distribute.
    /// use senders_receivers::refs;
    /// use std::fs::File;
    ///
    /// fn create_file(filename: String, buf: String) {
    ///     (
    ///         Just::from((filename, buf))
    ///         | Then::from(|(filename, buf)| {
    ///               match File::create(filename) {
    ///                   Ok(file) => Ok((file, buf)),
    ///                   Err(error) => Err(new_error(error)),
    ///               }
    ///           })
    ///         | LetValue::from(|sch, v: refs::ScopedRefMut<(File, String), refs::NoSendState>| {
    ///               let (file, buf) = DistributeRefTuple::distribute(v);
    ///               file.write_all(sch, refs::ScopedRefMut::map_no_mut(buf, |x| x.as_bytes()))
    ///           })
    ///     )
    ///         .sync_wait()
    ///         .unwrap()
    ///         .unwrap();
    ///     println!("wrote all the bytes!");
    /// }
    /// ```
    fn write_all(
        self,
        sch: Sch,
        buf: refs::ScopedRef<[u8], BufState>,
    ) -> WriteAllTS<Sch, Self::Target, SelfState, BufState>;
}

impl<Sch, T, SelfState, BufState> Write<Sch, SelfState, BufState>
    for refs::ScopedRefMut<T, SelfState>
where
    Sch: Scheduler,
    T: 'static + io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    fn write(
        self,
        sch: Sch,
        buf: refs::ScopedRef<[u8], BufState>,
    ) -> WriteTS<Sch, T, SelfState, BufState> {
        WriteTS { fd: self, buf, sch }
    }

    fn write_all(
        self,
        sch: Sch,
        buf: refs::ScopedRef<[u8], BufState>,
    ) -> WriteAllTS<Sch, T, SelfState, BufState> {
        WriteAllTS { fd: self, buf, sch }
    }
}

/// Typed-sender returned by the [Write] trait.
pub struct WriteTS<Sch, Fd, SelfState, BufState>
where
    Fd: 'static + io::Write + ?Sized,
    Sch: Scheduler,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    fd: refs::ScopedRefMut<Fd, SelfState>,
    buf: refs::ScopedRef<[u8], BufState>,
    sch: Sch,
}

impl<Sch, Fd, SelfState, BufState> TypedSender for WriteTS<Sch, Fd, SelfState, BufState>
where
    Fd: 'static + io::Write + ?Sized,
    Sch: Scheduler,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = (usize,);
}

impl<'a, ScopeImpl, ReceiverType, Sch, Fd, SelfState, BufState>
    TypedSenderConnect<'a, ScopeImpl, ReceiverType> for WriteTS<Sch, Fd, SelfState, BufState>
where
    Fd: 'static + io::Write + ?Sized,
    Sch: Scheduler + EnableDefaultIO,
    Sch::Sender: TypedSenderConnect<
        'a,
        ScopeImpl,
        ReceiverWrapper<ReceiverType, Sch::LocalScheduler, Fd, SelfState, BufState>,
    >,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, (usize,)>,
    ScopeImpl: ScopeWrap<
        Sch::LocalScheduler,
        ReceiverWrapper<ReceiverType, Sch::LocalScheduler, Fd, SelfState, BufState>,
    >,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
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

impl<BindSenderImpl, Sch, Fd, SelfState, BufState> BitOr<BindSenderImpl>
    for WriteTS<Sch, Fd, SelfState, BufState>
where
    BindSenderImpl: BindSender<Self>,
    Sch: Scheduler,
    Sch::Sender: TypedSender<Value = ()>,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<ReceiverType, Sch, Fd, SelfState, BufState>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    phantom: PhantomData<Sch>,
    nested: ReceiverType,
    fd: refs::ScopedRefMut<Fd, SelfState>,
    buf: refs::ScopedRef<[u8], BufState>,
}

impl<ReceiverType, Sch, Fd, SelfState, BufState> Receiver
    for ReceiverWrapper<ReceiverType, Sch, Fd, SelfState, BufState>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<Sch, ReceiverType, Fd, SelfState, BufState> ReceiverOf<Sch, ()>
    for ReceiverWrapper<ReceiverType, Sch, Fd, SelfState, BufState>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    fn set_value(mut self, sch: Sch, _: ()) {
        match (*self.fd).write(&self.buf) {
            Ok(len) => self.nested.set_value(sch, (len,)),
            Err(error) => self.nested.set_error(new_error(error)),
        };
    }
}

/// Typed-sender returned by the [Write] trait.
pub struct WriteAllTS<Sch, Fd, SelfState, BufState>
where
    Fd: 'static + io::Write + ?Sized,
    Sch: Scheduler,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    fd: refs::ScopedRefMut<Fd, SelfState>,
    buf: refs::ScopedRef<[u8], BufState>,
    sch: Sch,
}

impl<Sch, Fd, SelfState, BufState> TypedSender for WriteAllTS<Sch, Fd, SelfState, BufState>
where
    Fd: 'static + io::Write + ?Sized,
    Sch: Scheduler,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = ();
}

impl<'a, ScopeImpl, ReceiverType, Sch, Fd, SelfState, BufState>
    TypedSenderConnect<'a, ScopeImpl, ReceiverType> for WriteAllTS<Sch, Fd, SelfState, BufState>
where
    Fd: 'static + io::Write + ?Sized,
    Sch: Scheduler + EnableDefaultIO,
    Sch::Sender: TypedSenderConnect<
        'a,
        ScopeImpl,
        AllReceiverWrapper<ReceiverType, Sch::LocalScheduler, Fd, SelfState, BufState>,
    >,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, ()>,
    ScopeImpl: ScopeWrap<
        Sch::LocalScheduler,
        AllReceiverWrapper<ReceiverType, Sch::LocalScheduler, Fd, SelfState, BufState>,
    >,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
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
            AllReceiverWrapper {
                nested: receiver,
                fd: self.fd,
                buf: self.buf,
                phantom: PhantomData,
            },
        )
    }
}

impl<BindSenderImpl, Sch, Fd, SelfState, BufState> BitOr<BindSenderImpl>
    for WriteAllTS<Sch, Fd, SelfState, BufState>
where
    BindSenderImpl: BindSender<Self>,
    Sch: Scheduler,
    Sch::Sender: TypedSender<Value = ()>,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct AllReceiverWrapper<ReceiverType, Sch, Fd, SelfState, BufState>
where
    ReceiverType: ReceiverOf<Sch, ()>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    phantom: PhantomData<Sch>,
    nested: ReceiverType,
    fd: refs::ScopedRefMut<Fd, SelfState>,
    buf: refs::ScopedRef<[u8], BufState>,
}

impl<ReceiverType, Sch, Fd, SelfState, BufState> Receiver
    for AllReceiverWrapper<ReceiverType, Sch, Fd, SelfState, BufState>
where
    ReceiverType: ReceiverOf<Sch, ()>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<Sch, ReceiverType, Fd, SelfState, BufState> ReceiverOf<Sch, ()>
    for AllReceiverWrapper<ReceiverType, Sch, Fd, SelfState, BufState>
where
    ReceiverType: ReceiverOf<Sch, ()>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    SelfState: 'static + Clone + fmt::Debug,
    BufState: 'static + Clone + fmt::Debug,
{
    fn set_value(mut self, sch: Sch, _: ()) {
        match (*self.fd).write_all(&self.buf) {
            Ok(()) => self.nested.set_value(sch, ()),
            Err(error) => self.nested.set_error(new_error(error)),
        };
    }
}
