use crate::errors::Error;
use crate::scheduler::Scheduler;

/// Since we wrap everything in a function, we need to pass an enum to that function.
///
/// I would've preferred to use an [ReceiverOf](crate::traits::ReceiverOf) interface, but couldn't figure out how to make that work.
pub enum ScopeFnArgument<Sch>
where
    Sch: Scheduler<LocalScheduler = Sch>,
{
    Value(Sch),
    Error(Error),
    Done,
}
