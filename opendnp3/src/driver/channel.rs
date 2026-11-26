//! Minimal `no_std + alloc`, `#![forbid(unsafe_code)]` async unbounded MPSC channel.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use spin::Mutex;

#[derive(Debug)]
struct Inner<T> {
    queue: VecDeque<T>,
    waker: Option<Waker>,
    senders: usize,
    closed: bool,
}

/// Cloneable sender half of an unbounded `no_std` async channel.
#[derive(Debug)]
pub struct UnboundedSender<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        self.inner.lock().senders += 1;
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        let waker = {
            let mut guard = self.inner.lock();
            guard.senders -= 1;
            if guard.senders == 0 {
                guard.waker.take()
            } else {
                None
            }
        };
        if let Some(w) = waker {
            w.wake();
        }
    }
}

impl<T> UnboundedSender<T> {
    /// Push a value onto the channel without blocking.
    /// Returns `Err(value)` if the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<(), T> {
        let waker = {
            let mut guard = self.inner.lock();
            if guard.closed {
                return Err(value);
            }
            guard.queue.push_back(value);
            guard.waker.take()
        };
        if let Some(w) = waker {
            w.wake();
        }
        Ok(())
    }
}

/// Receiver half of an unbounded `no_std` async channel.
#[derive(Debug)]
pub struct UnboundedReceiver<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        self.inner.lock().closed = true;
    }
}

impl<T> UnboundedReceiver<T> {
    /// Receive the next item asynchronously, returning `None` when all senders
    /// have been dropped and the queue is empty.
    pub fn recv(&self) -> RecvFuture<'_, T> {
        RecvFuture { inner: &self.inner }
    }

    /// Try to pop an item without awaiting.
    pub fn try_recv(&self) -> Option<T> {
        self.inner.lock().queue.pop_front()
    }
}

pub struct RecvFuture<'a, T> {
    inner: &'a Mutex<Inner<T>>,
}

impl<T> Future for RecvFuture<'_, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut guard = self.inner.lock();
        if let Some(v) = guard.queue.pop_front() {
            Poll::Ready(Some(v))
        } else if guard.senders == 0 {
            Poll::Ready(None)
        } else {
            guard.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Create a new unbounded `no_std` async channel.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let inner = Arc::new(Mutex::new(Inner {
        queue: VecDeque::new(),
        waker: None,
        senders: 1,
        closed: false,
    }));
    (
        UnboundedSender {
            inner: Arc::clone(&inner),
        },
        UnboundedReceiver { inner },
    )
}
