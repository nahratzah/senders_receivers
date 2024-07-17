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

/// Implement the write trait for a [writeable](io::Write) type.
pub trait Write<Sch, State>
where
    Sch: Scheduler,
    Self: io::Write,
    State: 'static + Clone + fmt::Debug,
{
    /// Create a new sender-chain, which will write to this descriptor.
    ///
    /// ```
    /// use senders_receivers::io::{Write, WriteTS};
    /// use senders_receivers::{SyncWait, ImmediateScheduler, Just, LetValue, Then, Scheduler, TypedSender, Result};
    /// use senders_receivers::tuple::*; // for distribute.
    /// use senders_receivers::refs;
    /// use std::fs::File;
    ///
    /// fn do_the_thing<'a>(file: &'a mut File) {
    ///     let (wlen,) = (
    ///             Just::from((file, String::from("abcd"),))
    ///             | LetValue::from(|sch: ImmediateScheduler, v: refs::ScopedRefMut<(&'a mut File, String,), refs::NoSendState>| {
    ///                 let (mut file, s) = DistributeRefTuple::distribute(v);
    ///                 // XXX lifetimes.
    ///                 sch.lazy().schedule_value(
    ///                     file.write(sch.clone(), refs::ScopedRefMut::map_no_mut(s, |x| x.as_bytes())).sync_wait().unwrap().unwrap())
    ///               })
    ///             | Then::from(|(_, _, wlen)| (wlen,))
    ///         )
    ///         .sync_wait()
    ///         .unwrap()
    ///         .unwrap();
    ///     println!("wrote {} bytes", wlen);
    /// }
    /// ```
    fn write<'a>(
        &'a mut self,
        sch: Sch,
        buf: refs::ScopedRef<[u8], State>,
    ) -> WriteTS<'a, Sch, Self, State>;
}

impl<Sch, T, State> Write<Sch, State> for T
where
    Sch: Scheduler,
    T: io::Write + ?Sized,
    State: 'static + Clone + fmt::Debug,
{
    fn write<'a>(
        &'a mut self,
        sch: Sch,
        buf: refs::ScopedRef<[u8], State>,
    ) -> WriteTS<'a, Sch, Self, State> {
        WriteTS { fd: self, buf, sch }
    }
}

/// Typed-sender returned by the [Write] trait.
pub struct WriteTS<'a, Sch, Fd, State>
where
    Fd: io::Write + ?Sized,
    Sch: Scheduler,
    State: 'static + Clone + fmt::Debug,
{
    fd: &'a mut Fd,
    buf: refs::ScopedRef<[u8], State>,
    sch: Sch,
}

impl<'a, Sch, Fd, State> TypedSender for WriteTS<'a, Sch, Fd, State>
where
    Fd: io::Write + ?Sized,
    Sch: Scheduler,
    State: 'static + Clone + fmt::Debug,
{
    type Scheduler = Sch::LocalScheduler;
    type Value = (usize,);
}

impl<'a, ScopeImpl, ReceiverType, Sch, Fd, State> TypedSenderConnect<'a, ScopeImpl, ReceiverType>
    for WriteTS<'a, Sch, Fd, State>
where
    Fd: io::Write + ?Sized,
    Sch: Scheduler + EnableDefaultIO,
    Sch::Sender: TypedSenderConnect<
        'a,
        ScopeImpl,
        ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Fd, State>,
    >,
    ReceiverType: ReceiverOf<Sch::LocalScheduler, (usize,)>,
    ScopeImpl: ScopeWrap<
        Sch::LocalScheduler,
        ReceiverWrapper<'a, ReceiverType, Sch::LocalScheduler, Fd, State>,
    >,
    State: 'static + Clone + fmt::Debug,
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

impl<'a, BindSenderImpl, Sch, Fd, State> BitOr<BindSenderImpl> for WriteTS<'a, Sch, Fd, State>
where
    BindSenderImpl: BindSender<Self>,
    Sch: Scheduler,
    Sch::Sender: TypedSender<Value = ()>,
    Fd: io::Write + ?Sized,
    State: 'static + Clone + fmt::Debug,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

struct ReceiverWrapper<'a, ReceiverType, Sch, Fd, State>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    State: 'static + Clone + fmt::Debug,
{
    phantom: PhantomData<Sch>,
    nested: ReceiverType,
    fd: &'a mut Fd,
    buf: refs::ScopedRef<[u8], State>,
}

impl<'a, ReceiverType, Sch, Fd, State> Receiver
    for ReceiverWrapper<'a, ReceiverType, Sch, Fd, State>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    State: 'static + Clone + fmt::Debug,
{
    fn set_done(self) {
        self.nested.set_done();
    }

    fn set_error(self, error: Error) {
        self.nested.set_error(error);
    }
}

impl<'a, Sch, ReceiverType, Fd, State> ReceiverOf<Sch, ()>
    for ReceiverWrapper<'a, ReceiverType, Sch, Fd, State>
where
    ReceiverType: ReceiverOf<Sch, (usize,)>,
    Sch: Scheduler,
    Fd: io::Write + ?Sized,
    State: 'static + Clone + fmt::Debug,
{
    fn set_value(self, sch: Sch, _: ()) {
        match self.fd.write(&*self.buf) {
            Ok(len) => self.nested.set_value(sch, (len,)),
            Err(error) => self.nested.set_error(new_error(error)),
        };
    }
}
