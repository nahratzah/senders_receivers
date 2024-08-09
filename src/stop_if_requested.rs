use crate::errors::Error;
use crate::scheduler::Scheduler;
use crate::stop_token::StopToken;
use crate::traits::{BindSender, Receiver, ReceiverOf, Sender, TypedSender, TypedSenderConnect};
use crate::tuple::Tuple;
use std::ops::BitOr;

/// [Sender] that cancels further processing, if the [StopToken] requests such.
///
/// If the [StopToken] indicates that the operation should stop, then this sender
/// will replace the `value` signal with a `done` signal.
/// Errors are passed unchanged.
///
/// ```
/// use senders_receivers::{Just, Then, StopIfRequested, SyncWait};
///
/// let sender = Just::default()
/// | Then::from(|_: ()| ("do something",))
/// | StopIfRequested
/// | Then::from(|(s,): (&'static str,)| (s, "do something expensive"));
///
/// let _ = sender.sync_wait(); // XXX Would be nice if we actually had something
///                             // where cancelation is useful... but we don't (yet).
/// ```
#[derive(Default)]
pub struct StopIfRequested;

impl Sender for StopIfRequested {}

impl<TS> BindSender<TS> for StopIfRequested
where
    TS: TypedSender,
{
    type Output = StopIfRequestedTS<TS>;

    fn bind(self, ts: TS) -> Self::Output {
        StopIfRequestedTS { ts }
    }
}

pub struct StopIfRequestedTS<TS>
where
    TS: TypedSender,
{
    ts: TS,
}

impl<BindSenderImpl, TS> BitOr<BindSenderImpl> for StopIfRequestedTS<TS>
where
    TS: TypedSender,
    BindSenderImpl: BindSender<Self>,
{
    type Output = BindSenderImpl::Output;

    fn bitor(self, rhs: BindSenderImpl) -> Self::Output {
        rhs.bind(self)
    }
}

impl<TS> TypedSender for StopIfRequestedTS<TS>
where
    TS: TypedSender,
{
    type Scheduler = TS::Scheduler;
    type Value = TS::Value;
}

impl<'a, ScopeImpl, StopTokenImpl, Rcv, TS> TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, Rcv>
    for StopIfRequestedTS<TS>
where
    StopTokenImpl: 'a + StopToken,
    TS: TypedSenderConnect<'a, ScopeImpl, StopTokenImpl, ReceiverWrapper<StopTokenImpl, Rcv>>,
    Rcv: ReceiverOf<<TS as TypedSender>::Scheduler, <TS as TypedSender>::Value>,
{
    type Output<'scope> = TS::Output<'scope>
    where 'a: 'scope, ScopeImpl: 'scope, Rcv: 'scope;

    fn connect<'scope>(
        self,
        scope: &ScopeImpl,
        stop_token: StopTokenImpl,
        rcv: Rcv,
    ) -> Self::Output<'scope>
    where
        'a: 'scope,
        ScopeImpl: 'scope,
        Rcv: 'scope,
    {
        let rcv = ReceiverWrapper {
            stop_token: stop_token.clone(),
            rcv,
        };
        self.ts.connect(scope, stop_token, rcv)
    }
}

pub struct ReceiverWrapper<StopTokenImpl, Rcv>
where
    StopTokenImpl: StopToken,
    Rcv: Receiver,
{
    stop_token: StopTokenImpl,
    rcv: Rcv,
}

impl<StopTokenImpl, Rcv> Receiver for ReceiverWrapper<StopTokenImpl, Rcv>
where
    StopTokenImpl: StopToken,
    Rcv: Receiver,
{
    fn set_error(self, error: Error) {
        self.rcv.set_error(error)
    }

    fn set_done(self) {
        self.rcv.set_done()
    }
}

impl<Sch, Value, StopTokenImpl, Rcv> ReceiverOf<Sch, Value> for ReceiverWrapper<StopTokenImpl, Rcv>
where
    Sch: Scheduler<LocalScheduler = Sch>,
    Value: Tuple,
    StopTokenImpl: StopToken,
    Rcv: ReceiverOf<Sch, Value>,
{
    fn set_value(self, sch: Sch, value: Value) {
        if self.stop_token.stop_requested() {
            self.rcv.set_done();
        } else {
            self.rcv.set_value(sch, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StopIfRequested;
    use crate::errors::{new_error, ErrorForTesting};
    use crate::just::Just;
    use crate::just_done::JustDone;
    use crate::just_error::JustError;
    use crate::scheduler::ImmediateScheduler;
    use crate::sync_wait::SyncWait;

    #[test]
    fn it_propagates_value() {
        (Just::from((1, 2)) | StopIfRequested)
            .sync_wait()
            .expect("no error")
            .expect("no cancelation");
    }

    #[test]
    fn it_propagates_error() {
        let sender = JustError::<ImmediateScheduler, (i32, i32)>::from(new_error(
            ErrorForTesting::from("this error will be passed through"),
        )) | StopIfRequested;

        assert_eq!(
            ErrorForTesting::from("this error will be passed through"),
            *sender
                .sync_wait()
                .expect_err("should return the error")
                .downcast_ref::<ErrorForTesting>()
                .unwrap()
        )
    }

    #[test]
    fn it_propagates_done() {
        let sender = JustDone::<ImmediateScheduler, (i32, i32)>::default() | StopIfRequested;
        assert!(sender.sync_wait().expect("no error").is_none())
    }
}
