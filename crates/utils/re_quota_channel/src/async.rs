//! Asynchronous mpsc channel with byte-based backpressure.
//!
//! This wraps a [`tokio::sync::mpsc`] channel and adds byte-based capacity limits.
//! When the byte budget is exceeded, the sender will await until space is available.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use parking_lot::Mutex;
use tokio::sync::Notify;

// ----------------------------------------------------------------------------

/// A message together with its size in bytes.
pub struct SizedMessage<T> {
    pub msg: T,
    pub size_bytes: u64,
}

struct SharedState {
    debug_name: String,
    capacity_bytes: u64,

    /// Protected by mutex.
    current_bytes: Mutex<u64>,

    /// Signaled when bytes decrease (i.e., when a message is received).
    space_available: Notify,
}

// ----------------------------------------------------------------------------

/// Error returned when sending fails because the channel is disconnected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError<T>(pub T);

impl<T> std::fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "channel disconnected")
    }
}

impl<T: std::fmt::Debug> std::error::Error for SendError<T> {}

// ----------------------------------------------------------------------------

/// Error returned when receiving fails because the channel is disconnected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecvError;

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "channel disconnected")
    }
}

impl std::error::Error for RecvError {}

// ----------------------------------------------------------------------------

/// Error returned by [`AsyncSender::try_send`].
#[derive(Debug)]
pub enum TrySendError<T> {
    /// The channel's byte capacity is full.
    Full(T),

    /// The channel is disconnected.
    Disconnected(T),
}

impl<T> TrySendError<T> {
    /// Unwrap the message that couldn't be sent.
    pub fn into_inner(self) -> T {
        match self {
            Self::Full(msg) | Self::Disconnected(msg) => msg,
        }
    }

    /// Returns `true` if the channel is full.
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full(_))
    }

    /// Returns `true` if the channel is disconnected.
    pub fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected(_))
    }
}

impl<T> std::fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(_) => write!(f, "channel byte capacity exceeded"),
            Self::Disconnected(_) => write!(f, "channel disconnected"),
        }
    }
}

impl<T: std::fmt::Debug> std::error::Error for TrySendError<T> {}

// ----------------------------------------------------------------------------

/// Error returned by [`AsyncReceiver::try_recv`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TryRecvError {
    /// The channel is empty.
    Empty,

    /// The channel is disconnected.
    Disconnected,
}

impl std::fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "channel empty"),
            Self::Disconnected => write!(f, "channel disconnected"),
        }
    }
}

impl std::error::Error for TryRecvError {}

// ----------------------------------------------------------------------------

/// The sending end of a byte-bounded async channel.
///
/// Use [`AsyncSender::send`] to send messages with their byte size.
pub struct AsyncSender<T> {
    tx: tokio::sync::mpsc::UnboundedSender<SizedMessage<T>>,
    shared: Arc<SharedState>,
}

impl<T> Clone for AsyncSender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> AsyncSender<T>
where
    T: re_byte_size::SizeBytes,
{
    /// Send a message.
    ///
    /// This will await if the channel's byte capacity is exceeded,
    /// waiting until enough messages have been received to make room.
    ///
    /// If the message is larger than the channel's total capacity, a warning is logged
    /// and we wait until the channel is completely empty before sending.
    pub async fn send(&self, msg: T) -> Result<(), SendError<T>> {
        let size_bytes = msg.total_size_bytes();
        self.send_with_size(msg, size_bytes).await
    }

    /// Send a message, blocking the current thread until space is available.
    ///
    /// This is a synchronous wrapper around [`Self::send`] that blocks using tokio's
    /// `block_in_place`. Use this when you need to send from a synchronous context.
    ///
    /// # Panics
    ///
    /// Panics if called from outside a tokio runtime context.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn send_blocking(&self, msg: T) -> Result<(), SendError<T>> {
        let size_bytes = msg.total_size_bytes();
        self.send_with_size_blocking(msg, size_bytes)
    }
}

impl<T> AsyncSender<T> {
    /// Send a message with its size in bytes.
    ///
    /// This will await if the channel's byte capacity is exceeded,
    /// waiting until enough messages have been received to make room.
    ///
    /// If the message is larger than the channel's total capacity, a warning is logged
    /// and we wait until the channel is completely empty before sending.
    pub async fn send_with_size(&self, msg: T, size_bytes: u64) -> Result<(), SendError<T>> {
        let capacity = self.shared.capacity_bytes;

        // Special case: message is larger than total capacity
        if capacity < size_bytes {
            re_log::debug_once!(
                "{}: Message size ({}) exceeds channel capacity ({}). \
                 Waiting for channel to empty before sending.",
                self.shared.debug_name,
                re_format::format_bytes(size_bytes as f64),
                re_format::format_bytes(capacity as f64),
            );

            // Wait until the channel is completely empty
            loop {
                {
                    let current = self.shared.current_bytes.lock();
                    if *current == 0 {
                        break;
                    }
                }
                self.shared.space_available.notified().await;
            }

            // Now send (we'll temporarily exceed capacity, but that's expected)
            {
                let mut current = self.shared.current_bytes.lock();
                *current += size_bytes;
            }

            return self
                .tx
                .send(SizedMessage { msg, size_bytes })
                .map_err(|err| SendError(err.0.msg));
        }

        // Normal case: wait until we have room
        loop {
            {
                let mut current = self.shared.current_bytes.lock();
                if *current + size_bytes <= capacity {
                    *current += size_bytes;
                    break;
                }
            }
            self.shared.space_available.notified().await;
        }

        self.tx
            .send(SizedMessage { msg, size_bytes })
            .map_err(|err| SendError(err.0.msg))
    }

    /// Send a message with its size in bytes, blocking the current thread.
    ///
    /// This is a synchronous wrapper around [`Self::send_with_size`] that blocks using
    /// tokio's `block_in_place`. Use this when you need to send from a synchronous context.
    ///
    /// # Panics
    ///
    /// Panics if called from outside a tokio runtime context.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn send_with_size_blocking(&self, msg: T, size_bytes: u64) -> Result<(), SendError<T>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.send_with_size(msg, size_bytes))
        })
    }

    /// Try to send a message without awaiting.
    ///
    /// Returns an error if the channel is full (by byte count) or disconnected.
    pub fn try_send(&self, msg: T, size_bytes: u64) -> Result<(), TrySendError<T>> {
        let capacity = self.shared.capacity_bytes;

        let mut current = self.shared.current_bytes.lock();

        // Check if we have room (allow oversized messages if channel is empty)
        if capacity < *current + size_bytes && 0 < *current {
            return Err(TrySendError::Full(msg));
        }

        *current += size_bytes;
        drop(current);

        self.tx
            .send(SizedMessage { msg, size_bytes })
            .map_err(|err| TrySendError::Disconnected(err.0.msg))
    }

    /// Returns the debug name of the channel.
    #[inline]
    pub fn debug_name(&self) -> &str {
        &self.shared.debug_name
    }

    /// Returns `true` if the channel is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        *self.shared.current_bytes.lock() == 0
    }

    /// Returns the current byte usage in the channel.
    #[inline]
    pub fn current_bytes(&self) -> u64 {
        *self.shared.current_bytes.lock()
    }

    /// Returns the byte capacity of the channel.
    #[inline]
    pub fn capacity_bytes(&self) -> u64 {
        self.shared.capacity_bytes
    }
}

// ----------------------------------------------------------------------------

/// The receiving end of a byte-bounded async channel.
pub struct AsyncReceiver<T> {
    rx: tokio::sync::mpsc::UnboundedReceiver<SizedMessage<T>>,
    shared: Arc<SharedState>,
}

impl<T> AsyncReceiver<T> {
    /// Receive a message, awaiting until one is available.
    ///
    /// Returns `None` if the channel is closed and empty.
    pub async fn recv(&mut self) -> Option<T> {
        let sized_msg = self.rx.recv().await?;
        self.on_receive(sized_msg.size_bytes);
        Some(sized_msg.msg)
    }

    /// Try to receive a message without awaiting.
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        match self.rx.try_recv() {
            Ok(sized_msg) => {
                self.on_receive(sized_msg.size_bytes);
                Ok(sized_msg.msg)
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => Err(TryRecvError::Empty),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                Err(TryRecvError::Disconnected)
            }
        }
    }

    /// Close the receiving half of the channel.
    ///
    /// This prevents any further messages from being sent on the channel.
    pub fn close(&mut self) {
        self.rx.close();
    }

    /// Returns the debug name of the channel.
    #[inline]
    pub fn debug_name(&self) -> &str {
        &self.shared.debug_name
    }

    /// Returns `true` if the channel is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }

    /// Returns the number of messages in the channel.
    #[inline]
    pub fn len(&self) -> usize {
        self.rx.len()
    }

    /// Returns the current byte usage in the channel.
    #[inline]
    pub fn current_bytes(&self) -> u64 {
        *self.shared.current_bytes.lock()
    }

    /// Returns the byte capacity of the channel.
    #[inline]
    pub fn capacity_bytes(&self) -> u64 {
        self.shared.capacity_bytes
    }

    fn on_receive(&self, size_bytes: u64) {
        let mut current = self.shared.current_bytes.lock();
        *current = current.saturating_sub(size_bytes);
        drop(current);
        self.shared.space_available.notify_waiters();
    }
}

impl<T> futures::Stream for AsyncReceiver<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(sized_msg)) => {
                self.on_receive(sized_msg.size_bytes);
                Poll::Ready(Some(sized_msg.msg))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ----------------------------------------------------------------------------

/// Create a new byte-bounded async channel.
///
/// # Arguments
///
/// * `debug_name` - A name used for logging messages about this channel.
/// * `capacity_bytes` - The maximum number of bytes allowed in the channel before
///   backpressure is applied. [`AsyncSender::send`] will await when this limit is reached.
pub fn async_channel<T>(
    debug_name: impl Into<String>,
    capacity_bytes: u64,
) -> (AsyncSender<T>, AsyncReceiver<T>) {
    #[expect(clippy::disallowed_methods)] // We bound it manually, to max byte size
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let shared = Arc::new(SharedState {
        debug_name: debug_name.into(),
        capacity_bytes,
        current_bytes: Mutex::new(0),
        space_available: Notify::new(),
    });

    let sender = AsyncSender {
        tx,
        shared: Arc::clone(&shared),
    };

    let receiver = AsyncReceiver { rx, shared };

    (sender, receiver)
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_send_recv() {
        let (tx, mut rx) = async_channel::<String>("test".to_owned(), 1000);

        tx.send_with_size("hello".to_owned(), 5).await.unwrap();
        tx.send_with_size("world".to_owned(), 5).await.unwrap();

        assert_eq!(rx.recv().await.unwrap(), "hello");
        assert_eq!(rx.recv().await.unwrap(), "world");
    }

    #[tokio::test]
    async fn test_byte_tracking() {
        let (tx, mut rx) = async_channel::<String>("test".to_owned(), 1000);

        assert_eq!(tx.current_bytes(), 0);

        tx.send_with_size("hello".to_owned(), 100).await.unwrap();
        assert_eq!(tx.current_bytes(), 100);

        tx.send_with_size("world".to_owned(), 200).await.unwrap();
        assert_eq!(tx.current_bytes(), 300);

        let _ = rx.recv().await.unwrap();
        assert_eq!(rx.current_bytes(), 200);

        let _ = rx.recv().await.unwrap();
        assert_eq!(rx.current_bytes(), 0);
    }

    #[tokio::test]
    async fn test_try_send_when_full() {
        let (tx, _rx) = async_channel::<String>("test".to_owned(), 100);

        // First message fits
        assert!(tx.try_send("hello".to_owned(), 80).is_ok());

        // Second message doesn't fit
        let result = tx.try_send("world".to_owned(), 80);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_full());
    }

    #[tokio::test]
    async fn test_oversized_message_when_empty() {
        let (tx, mut rx) = async_channel::<String>("test".to_owned(), 100);

        // Oversized message should succeed when channel is empty (via try_send)
        assert!(tx.try_send("huge".to_owned(), 200).is_ok());
        assert_eq!(rx.recv().await.unwrap(), "huge");
    }

    #[tokio::test]
    async fn test_async_backpressure() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let (tx, mut rx) = async_channel::<u32>("test".to_owned(), 100);
        let send_completed = Arc::new(AtomicBool::new(false));
        let send_completed_clone = Arc::clone(&send_completed);

        // Fill up the channel
        tx.send_with_size(1, 80).await.unwrap();

        // Spawn a task that will try to send another message
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            tx_clone.send_with_size(2, 80).await.unwrap(); // This should await
            send_completed_clone.store(true, Ordering::SeqCst);
        });

        // Give the task time to start and await
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // The send should not have completed yet
        assert!(!send_completed.load(Ordering::SeqCst));

        // Now receive a message to free up space
        let _ = rx.recv().await.unwrap();

        // Wait for the send to complete
        handle.await.unwrap();
        assert!(send_completed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_channel_close() {
        let (tx, mut rx) = async_channel::<String>("test".to_owned(), 1000);

        tx.send_with_size("hello".to_owned(), 5).await.unwrap();
        drop(tx);

        // Should still receive the pending message
        assert_eq!(rx.recv().await.unwrap(), "hello");

        // Channel is now closed and empty
        assert!(rx.recv().await.is_none());
    }
}
