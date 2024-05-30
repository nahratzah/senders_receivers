use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::ops::Drop;
use std::rc::Rc;
use std::sync::mpsc;

// Re-export the errors from mpsc.
// The idea is to re-use the look of the mpsc interface.
pub use mpsc::{RecvError, SendError, TryRecvError};

/// Channel-receiver.
///
/// Allows receiving values that are added to the channel.
pub struct Receiver<T> {
    channel: Rc<RefCell<Channel<T>>>,
}

/// Channel-sender.
///
/// Allows sending values on the channel.
pub struct Sender<T> {
    channel: Rc<RefCell<Channel<T>>>,
}

/// Create a new channel. The created channel cannot cross thread-boundaries.
///
/// This mirrors the [mpsc::channel](std::sync::mpsc::channel) interface.
///
/// The returned channel will have an initial `capacity`.
/// The channel will grow to accommodate more elements.
pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let channel = Rc::new(RefCell::new(Channel::new(capacity)));
    (
        Sender {
            channel: channel.clone(),
        },
        Receiver {
            channel: channel.clone(),
        },
    )
}

/// Internal channel object.
struct Channel<T> {
    /// Track if the channel has a receiver.
    has_receiver: bool,
    /// Track how many senders the channel has.
    cnt_sender: usize,
    /// Queue of objects.
    queue: VecDeque<T>,
}

impl<T> Channel<T> {
    fn new(capacity: usize) -> Channel<T> {
        Channel {
            has_receiver: true,
            cnt_sender: 1,
            queue: VecDeque::with_capacity(capacity),
        }
    }

    fn push(&mut self, v: T) -> Result<(), SendError<T>> {
        assert!(self.cnt_sender > 0);
        if !self.has_receiver {
            return Err(SendError(v));
        }
        self.queue.push_back(v);
        Ok(())
    }

    fn pop(&mut self) -> Result<T, RecvError> {
        assert!(self.has_receiver);
        self.queue.pop_front().ok_or(RecvError)
    }

    fn try_pop(&mut self) -> Result<T, TryRecvError> {
        assert!(self.has_receiver);
        self.queue.pop_front().ok_or(match self.cnt_sender {
            0 => TryRecvError::Disconnected,
            _ => TryRecvError::Empty,
        })
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
        assert!(channel.has_receiver);
        channel.has_receiver = false;
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
        assert!(channel.cnt_sender > 0);
        channel.cnt_sender -= 1;
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        {
            let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
            channel.cnt_sender += 1;
        }
        Sender {
            channel: self.channel.clone(),
        }
    }
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<T, RecvError> {
        let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
        channel.pop()
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
        channel.try_pop()
    }

    pub fn is_empty(&self) -> bool {
        let channel: RefMut<'_, _> = self.channel.borrow_mut();
        channel.is_empty()
    }
}

impl<T> Sender<T> {
    pub fn send(&self, v: T) -> Result<(), SendError<T>> {
        let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
        channel.push(v)
    }
}

#[cfg(test)]
mod tests {
    use super::{channel, RecvError, SendError};

    #[test]
    fn it_works() {
        let (tx, rx) = channel(1);
        tx.send("bla").expect("send to succeed");
        assert_eq!("bla", rx.recv().expect("receive to succeed"));
    }

    #[test]
    fn empty_queue_yields_error() {
        let (tx, rx) = channel::<String>(1);
        assert_eq!(RecvError, rx.recv().expect_err("receive to fail"));
        drop(tx)
    }

    #[test]
    fn sender_disconnected_queue_yields_error() {
        let (tx, rx) = channel::<String>(1);
        drop(tx);
        assert_eq!(RecvError, rx.recv().expect_err("receive to fail"));
    }

    #[test]
    fn receiver_disconnected_queue_yields_error() {
        let (tx, rx) = channel::<String>(1);
        drop(rx);
        assert_eq!(
            SendError(String::from("nope nope nope")),
            tx.send(String::from("nope nope nope"))
                .expect_err("send to fail")
        );
    }
}
