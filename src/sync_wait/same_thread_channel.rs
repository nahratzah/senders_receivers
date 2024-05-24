use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::ops::Drop;
use std::rc::Rc;
use std::sync::mpsc;

pub type SendError<T> = mpsc::SendError<T>;
pub type RecvError = mpsc::RecvError;

pub struct Receiver<T> {
    channel: Rc<RefCell<Channel<T>>>,
}

#[derive(Clone)]
pub struct Sender<T> {
    channel: Rc<RefCell<Channel<T>>>,
}

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

struct Channel<T> {
    has_receiver: bool,
    has_sender: bool,
    queue: VecDeque<T>,
}

impl<T> Channel<T> {
    fn new(capacity: usize) -> Channel<T> {
        Channel {
            has_receiver: true,
            has_sender: true,
            queue: VecDeque::with_capacity(capacity),
        }
    }

    fn push(&mut self, v: T) -> Result<(), SendError<T>> {
        assert!(self.has_sender);
        if !self.has_receiver {
            return Err(mpsc::SendError(v));
        }
        self.queue.push_back(v);
        Ok(())
    }

    fn pop(&mut self) -> Result<T, RecvError> {
        assert!(self.has_receiver);
        self.queue.pop_front().ok_or(mpsc::RecvError)
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
        assert!(channel.has_sender);
        channel.has_sender = false;
    }
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<T, RecvError> {
        let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
        channel.pop()
    }
}

impl<T> Sender<T> {
    pub fn send(&self, v: T) -> Result<(), SendError<T>> {
        let mut channel: RefMut<'_, _> = self.channel.borrow_mut();
        channel.push(v)
    }
}
