//! Thin wrapper around [`tokio::sync::mpsc`] that logs a warning if send blocks too long.
//!
//! This is useful for debugging backpressure issues in async pipelines.

use tokio::sync::mpsc;

use crate::{BLOCKED_WARNING_THRESHOLD, BLOCKED_WARNING_THRESHOLD_SECS};

/// A sender for an mpsc channel that logs a warning if send blocks too long.
pub struct Sender<T> {
    inner: mpsc::Sender<T>,
    debug_name: String,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            debug_name: self.debug_name.clone(),
        }
    }
}

impl<T: Send> Sender<T> {
    /// Send a value asynchronously, logging a warning if blocked for more than 5 seconds.
    ///
    /// Returns `Err` if the receiver has been dropped.
    pub async fn send(&self, value: T) -> Result<(), mpsc::error::SendError<T>> {
        // First try to reserve a permit, which is what actually blocks
        loop {
            match tokio::time::timeout(BLOCKED_WARNING_THRESHOLD, self.inner.reserve()).await {
                Ok(Ok(permit)) => {
                    permit.send(value);
                    return Ok(());
                }
                Ok(Err(_)) => {
                    // Channel closed
                    return Err(mpsc::error::SendError(value));
                }
                Err(_timeout) => {
                    re_log::warn_once!(
                        "{}: Sender has been blocked for more than {BLOCKED_WARNING_THRESHOLD_SECS} seconds",
                        self.debug_name,
                    );
                    // Continue waiting
                }
            }
        }
    }

    /// Send a value, blocking the current thread if the channel is full.
    ///
    /// Logs a warning if blocked for more than 5 seconds.
    ///
    /// Returns `Err` if the receiver has been dropped.
    ///
    /// # Panics
    ///
    /// Panics if called from outside a tokio runtime context.
    pub fn blocking_send(&self, value: T) -> Result<(), mpsc::error::SendError<T>> {
        tokio::runtime::Handle::current().block_on(self.send(value))
    }
}

/// A receiver for an mpsc channel.
pub struct Receiver<T> {
    inner: mpsc::Receiver<T>,
}

impl<T> Receiver<T> {
    /// Receive the next value.
    ///
    /// Returns `None` if all senders have been dropped.
    pub async fn recv(&mut self) -> Option<T> {
        self.inner.recv().await
    }
}

/// Create a new mpsc channel with a debug name.
///
/// The `debug_name` is used in warning messages if send blocks for too long.
pub fn channel<T>(debug_name: impl Into<String>, capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel(capacity);
    (
        Sender {
            inner: tx,
            debug_name: debug_name.into(),
        },
        Receiver { inner: rx },
    )
}
