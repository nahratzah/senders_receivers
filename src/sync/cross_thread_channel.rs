use mio::{Poll, Token, Waker};
use std::collections::VecDeque;
use std::io;
use std::ops::Drop;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

// Re-export the errors from mpsc.
// The idea is to re-use the look of the mpsc interface.
pub use mpsc::{SendError, TryRecvError};

/// Channel-receiver.
///
/// Allows receiving values that are added to the channel.
pub struct Receiver<T: Send + 'static> {
    channel: Arc<Mutex<Channel<T>>>,
}

/// Channel-sender.
///
/// Allows sending values on the channel.
pub struct Sender<T: Send + 'static> {
    channel: Arc<Mutex<Channel<T>>>,
}

/// Create a new channel. The created channel cannot cross thread-boundaries.
///
/// This mirrors the [mpsc::channel](std::sync::mpsc::channel) interface.
///
/// The returned channel will have an initial `capacity`.
/// The channel will grow to accommodate more elements.
pub fn channel<T: Send + 'static>(
    poll: &Poll,
    token: Token,
) -> Result<(Sender<T>, Receiver<T>), io::Error> {
    let channel = Arc::new(Mutex::new(Channel::new(poll, token)?));
    Ok((
        Sender {
            channel: channel.clone(),
        },
        Receiver {
            channel: channel.clone(),
        },
    ))
}

/// Internal channel object.
struct Channel<T: Send + 'static> {
    /// Waker to wake up the worker thread.
    waker: Waker,
    /// Track if the channel has a receiver.
    has_receiver: bool,
    /// Track how many senders the channel has.
    cnt_sender: usize,
    /// Queue of objects.
    queue: VecDeque<T>,
}

impl<T: Send + 'static> Channel<T> {
    fn new(poll: &Poll, token: Token) -> Result<Channel<T>, io::Error> {
        Ok(Channel {
            waker: Waker::new(poll.registry(), token)?,
            has_receiver: true,
            cnt_sender: 1,
            queue: VecDeque::new(),
        })
    }

    fn push(&mut self, v: T) -> Result<(), SendError<T>> {
        if self.waker.wake().is_err() {
            return Err(SendError(v));
        }

        assert!(self.cnt_sender > 0);
        if !self.has_receiver {
            return Err(SendError(v));
        }
        self.queue.push_back(v);
        Ok(())
    }

    fn try_pop(&mut self) -> Result<T, TryRecvError> {
        assert!(self.has_receiver);
        self.queue.pop_front().ok_or(match self.cnt_sender {
            0 => TryRecvError::Disconnected,
            _ => TryRecvError::Empty,
        })
    }
}

impl<T: Send + 'static> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut channel = self.channel.lock().expect("mutex should lock just fine");
        assert!(channel.has_receiver);
        channel.has_receiver = false;
    }
}

impl<T: Send + 'static> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut channel = self.channel.lock().expect("mutex should lock just fine");
        assert!(channel.cnt_sender > 0);
        channel.cnt_sender -= 1;
        if channel.cnt_sender == 0 {
            channel.waker.wake().expect("wakeup should succeed"); // We can't process the error.
        }
    }
}

impl<T: Send + 'static> Clone for Sender<T> {
    fn clone(&self) -> Self {
        {
            let mut channel = self.channel.lock().expect("mutex should lock just fine");
            channel.cnt_sender += 1;
        }
        Sender {
            channel: self.channel.clone(),
        }
    }
}

impl<T: Send + 'static> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut channel = self.channel.lock().expect("mutex should lock just fine");
        channel.try_pop()
    }
}

impl<T: Send + 'static> Sender<T> {
    pub fn send(&self, v: T) -> Result<(), SendError<T>> {
        let mut channel = self.channel.lock().expect("mutex should lock just fine");
        channel.push(v)
    }
}

#[cfg(test)]
mod tests {
    use super::{channel, SendError, TryRecvError};
    use mio::{Events, Poll, Token};
    use std::time::Duration;
    const WAKEUP_TOKEN: Token = Token(0);

    fn received_wakup_token(poll: &mut Poll) -> bool {
        let mut events = Events::with_capacity(10);

        poll.poll(&mut events, Some(Duration::from_secs(0)))
            .unwrap();
        for event in &events {
            if event.token() == WAKEUP_TOKEN {
                return true;
            }
        }
        false
    }

    #[test]
    fn it_works() {
        let mut poll = Poll::new().unwrap();

        let (tx, rx) = channel(&poll, WAKEUP_TOKEN).unwrap();
        tx.send("bla").expect("send to succeed");
        assert_eq!("bla", rx.try_recv().expect("receive to succeed"));

        assert!(received_wakup_token(&mut poll));
    }

    #[test]
    fn empty_queue_yields_error() {
        let mut poll = Poll::new().unwrap();

        let (tx, rx) = channel::<String>(&poll, WAKEUP_TOKEN).unwrap();
        assert_eq!(
            TryRecvError::Empty,
            rx.try_recv().expect_err("receive to fail")
        );

        // We expect no wakeup event to be set.
        assert!(!received_wakup_token(&mut poll));

        drop(tx);
    }

    #[test]
    fn sender_disconnected_queue_yields_error() {
        let mut poll = Poll::new().unwrap();

        let (tx, rx) = channel::<String>(&poll, WAKEUP_TOKEN).unwrap();
        drop(tx);
        assert_eq!(
            TryRecvError::Disconnected,
            rx.try_recv().expect_err("receive to fail")
        );

        assert!(
            received_wakup_token(&mut poll),
            "disconnect should cause a wakeup"
        );
    }

    #[test]
    fn sender_disconnected_queue_yields_error_only_when_drained() {
        let mut poll = Poll::new().unwrap();

        let (tx, rx) = channel::<String>(&poll, WAKEUP_TOKEN).unwrap();
        tx.send(String::from("must receive this")).unwrap();
        drop(tx);
        assert_eq!(
            String::from("must receive this"),
            rx.try_recv()
                .expect("no error, because there is an enqueue item")
        );
        assert_eq!(
            TryRecvError::Disconnected,
            rx.try_recv().expect_err("receive to fail")
        );

        assert!(received_wakup_token(&mut poll));
    }

    #[test]
    fn receiver_disconnected_queue_yields_error() {
        let poll = Poll::new().unwrap();

        let (tx, rx) = channel::<String>(&poll, WAKEUP_TOKEN).unwrap();
        drop(rx);
        assert_eq!(
            SendError(String::from("nope nope nope")),
            tx.send(String::from("nope nope nope"))
                .expect_err("send to fail")
        );
    }
}
