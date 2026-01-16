//! A mpsc channel that applies backpressure based on byte size.
//!
//! This wraps a [`crossbeam::channel`] and adds byte-based capacity limits.
//! When the byte budget is exceeded, the sender will block until space is available.
//!
//! On `wasm32`, blocking is not allowed, so we only log a warning when the budget is exceeded.

use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

pub use crossbeam::channel::{RecvError, RecvTimeoutError, SendError, TryRecvError};

// ----------------------------------------------------------------------------

/// A message together with its size in bytes.
pub struct SizedMessage<T> {
    pub msg: T,
    pub size_bytes: u64,
}

struct SharedState {
    debug_name: String,
    capacity_bytes: u64,

    /// Protected by mutex for use with condvar.
    current_bytes: Mutex<u64>,

    /// Signaled when bytes decrease (i.e., when a message is received).
    #[cfg(not(target_arch = "wasm32"))]
    space_available: Condvar,
}

// ----------------------------------------------------------------------------

/// The sending end of a byte-bounded channel.
///
/// Use [`Sender::send`] to send messages with their byte size.
pub struct Sender<T> {
    tx: crossbeam::channel::Sender<SizedMessage<T>>,
    shared: Arc<SharedState>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> Sender<T>
where
    T: re_byte_size::SizeBytes,
{
    /// Send a message.
    ///
    /// On native platforms, this will block if the channel's byte capacity is exceeded,
    /// waiting until enough messages have been received to make room.
    ///
    /// If the message is larger than the channel's total capacity, a warning is logged
    /// and we wait until the channel is completely empty before sending.
    ///
    /// On `wasm32`, blocking is not possible, so we only log a warning when over capacity.
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        let size_bytes = msg.total_size_bytes();
        self.send_with_size(msg, size_bytes)
    }
}

impl<T> Sender<T> {
    /// Send a message with its size in bytes.
    ///
    /// On native platforms, this will block if the channel's byte capacity is exceeded,
    /// waiting until enough messages have been received to make room.
    ///
    /// If the message is larger than the channel's total capacity, a warning is logged
    /// and we wait until the channel is completely empty before sending.
    ///
    /// On `wasm32`, blocking is not possible, so we only log a warning when over capacity.
    pub fn send_with_size(&self, msg: T, size_bytes: u64) -> Result<(), SendError<T>> {
        self.send_impl(msg, size_bytes)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn send_impl(&self, msg: T, size_bytes: u64) -> Result<(), SendError<T>> {
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
            let mut current = self.shared.current_bytes.lock();
            while 0 < *current {
                self.shared.space_available.wait(&mut current);
            }

            // Now send (we'll temporarily exceed capacity, but that's expected)
            *current += size_bytes;
            drop(current);

            return self
                .tx
                .send(SizedMessage { msg, size_bytes })
                .map_err(|err| SendError(err.0.msg));
        }

        // Normal case: wait until we have room
        let mut current = self.shared.current_bytes.lock();
        while capacity < *current + size_bytes {
            self.shared.space_available.wait(&mut current);
        }

        *current += size_bytes;
        drop(current);

        self.tx
            .send(SizedMessage { msg, size_bytes })
            .map_err(|err| SendError(err.0.msg))
    }

    #[cfg(target_arch = "wasm32")]
    fn send_impl(&self, msg: T, size_bytes: u64) -> Result<(), SendError<T>> {
        let capacity = self.shared.capacity_bytes;

        let mut current = self.shared.current_bytes.lock();
        let new_total = *current + size_bytes;

        if capacity < new_total {
            re_log::debug_once!(
                "{}: Channel byte budget ({}) exceeded. \
                 Cannot block on web, sending anyway.",
                self.shared.debug_name,
                re_format::format_bytes(capacity as f64),
            );
        }

        *current = new_total;
        drop(current);

        self.tx
            .send(SizedMessage { msg, size_bytes })
            .map_err(|err| SendError(err.0.msg))
    }

    /// Try to send a message without blocking.
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
        self.tx.is_empty()
    }

    /// Returns the number of messages in the channel.
    #[inline]
    pub fn len(&self) -> usize {
        self.tx.len()
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

/// Error returned by [`Sender::try_send`].
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

/// The receiving end of a byte-bounded channel.
#[derive(Clone)]
pub struct Receiver<T> {
    rx: crossbeam::channel::Receiver<SizedMessage<T>>,
    shared: Arc<SharedState>,
}

impl<T> Receiver<T> {
    /// Receive a message, blocking until one is available.
    pub fn recv(&self) -> Result<T, RecvError> {
        let sized_msg = self.rx.recv()?;
        self.manual_on_receive(sized_msg.size_bytes);
        Ok(sized_msg.msg)
    }

    /// Receive a message with a timeout.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<T, RecvTimeoutError> {
        let sized_msg = self.rx.recv_timeout(timeout)?;
        self.manual_on_receive(sized_msg.size_bytes);
        Ok(sized_msg.msg)
    }

    /// Try to receive a message without blocking.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let sized_msg = self.rx.try_recv()?;
        self.manual_on_receive(sized_msg.size_bytes);
        Ok(sized_msg.msg)
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

    /// Create an iterator that receives messages.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.recv().ok())
    }

    /// Access the inner crossbeam receiver.
    ///
    /// This can be useful for use with `crossbeam::select!`.
    ///
    /// WARNING: if you receive a message directly from this receiver,
    /// you must manually call `on_receive` to update the byte count!
    pub fn manual_inner(&self) -> &crossbeam::channel::Receiver<SizedMessage<T>> {
        &self.rx
    }

    /// For use with [`manual_inner`]: notify the channel that a message of the given size
    /// has been received, so that the byte count can be updated.
    pub fn manual_on_receive(&self, size_bytes: u64) {
        let mut current = self.shared.current_bytes.lock();
        *current = current.saturating_sub(size_bytes);

        #[cfg(not(target_arch = "wasm32"))]
        {
            drop(current);
            self.shared.space_available.notify_all();
        }
    }
}

// ----------------------------------------------------------------------------

/// Create a new byte-bounded channel.
///
/// # Arguments
///
/// * `debug_name` - A name used for logging messages about this channel.
/// * `capacity_bytes` - The maximum number of bytes allowed in the channel before
///   backpressure is applied. On native platforms, [`Sender::send`] will block
///   when this limit is reached. On `wasm32`, a warning is logged but sending continues.
pub fn channel<T>(debug_name: impl Into<String>, capacity_bytes: u64) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = crossbeam::channel::unbounded();

    let shared = Arc::new(SharedState {
        debug_name: debug_name.into(),
        capacity_bytes,
        current_bytes: Mutex::new(0),
        #[cfg(not(target_arch = "wasm32"))]
        space_available: Condvar::new(),
    });

    let sender = Sender {
        tx,
        shared: Arc::clone(&shared),
    };

    let receiver = Receiver { rx, shared };

    (sender, receiver)
}

/// Wait on two receivers and execute whichever branch becomes ready first.
///
/// This is a simplified version of `crossbeam::select!` that only supports
/// two `recv` operations. It properly handles the byte accounting by calling
/// `manual_on_receive` after each successful receive.
///
/// # Syntax
///
/// ```ignore
/// select! {
///     recv(rx1) -> result => { /* handle result */ },
///     recv(rx2) -> result => { /* handle result */ },
/// }
/// ```
///
/// # Example
///
/// ```
/// use re_quota_channel::{channel, select};
///
/// let (tx1, rx1) = channel::<i32>("chan1", 1024);
/// let (tx2, rx2) = channel::<String>("chan2", 1024);
///
/// tx1.send_with_size(42, 8).unwrap();
///
/// select! {
///     recv(rx1) -> res => {
///         assert_eq!(res.unwrap(), 42);
///     },
///     recv(rx2) -> res => {
///         panic!("unexpected");
///     },
/// }
/// ```
#[macro_export]
macro_rules! select {
    (
        recv($rx1:expr) -> $res1:tt => $body1:block
        recv($rx2:expr) -> $res2:tt => $body2:block
    ) => {{
        let __rx1 = &$rx1;
        let __rx2 = &$rx2;
        ::crossbeam::channel::select! {
            recv(__rx1.manual_inner()) -> __result => {
                let $res1 = __result.map(|__sized| {
                    __rx1.manual_on_receive(__sized.size_bytes);
                    __sized.msg
                });
                $body1
            }
            recv(__rx2.manual_inner()) -> __result => {
                let $res2 = __result.map(|__sized| {
                    __rx2.manual_on_receive(__sized.size_bytes);
                    __sized.msg
                });
                $body2
            }
        }
    }};

    // Also support comma-separated format
    (
        recv($rx1:expr) -> $res1:tt => $body1:expr,
        recv($rx2:expr) -> $res2:tt => $body2:expr $(,)?
    ) => {{
        let __rx1 = &$rx1;
        let __rx2 = &$rx2;
        ::crossbeam::channel::select! {
            recv(__rx1.manual_inner()) -> __result => {
                let $res1 = __result.map(|__sized| {
                    __rx1.manual_on_receive(__sized.size_bytes);
                    __sized.msg
                });
                $body1
            }
            recv(__rx2.manual_inner()) -> __result => {
                let $res2 = __result.map(|__sized| {
                    __rx2.manual_on_receive(__sized.size_bytes);
                    __sized.msg
                });
                $body2
            }
        }
    }};
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_send_recv() {
        let (tx, rx) = channel::<String>("test".to_owned(), 1000);

        tx.send_with_size("hello".to_owned(), 5).unwrap();
        tx.send_with_size("world".to_owned(), 5).unwrap();

        assert_eq!(rx.recv().unwrap(), "hello");
        assert_eq!(rx.recv().unwrap(), "world");
    }

    #[test]
    fn test_byte_tracking() {
        let (tx, rx) = channel::<String>("test".to_owned(), 1000);

        assert_eq!(tx.current_bytes(), 0);

        tx.send_with_size("hello".to_owned(), 100).unwrap();
        assert_eq!(tx.current_bytes(), 100);

        tx.send_with_size("world".to_owned(), 200).unwrap();
        assert_eq!(tx.current_bytes(), 300);

        let _ = rx.recv().unwrap();
        assert_eq!(rx.current_bytes(), 200);

        let _ = rx.recv().unwrap();
        assert_eq!(rx.current_bytes(), 0);
    }

    #[test]
    fn test_try_send_when_full() {
        let (tx, _rx) = channel::<String>("test".to_owned(), 100);

        // First message fits
        assert!(tx.try_send("hello".to_owned(), 80).is_ok());

        // Second message doesn't fit
        let result = tx.try_send("world".to_owned(), 80);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_full());
    }

    #[test]
    fn test_oversized_message_when_empty() {
        let (tx, rx) = channel::<String>("test".to_owned(), 100);

        // Oversized message should succeed when channel is empty (via try_send)
        assert!(tx.try_send("huge".to_owned(), 200).is_ok());
        assert_eq!(rx.recv().unwrap(), "huge");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_blocking_backpressure() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::Duration;

        let (tx, rx) = channel::<u32>("test".to_owned(), 100);
        let send_completed = Arc::new(AtomicBool::new(false));
        let send_completed_clone = Arc::clone(&send_completed);

        // Fill up the channel
        tx.send_with_size(1, 80).unwrap();

        // Spawn a thread that will try to send another message
        let tx_clone = tx.clone();

        #[expect(clippy::disallowed_methods)] // It's only a test
        let handle = std::thread::spawn(move || {
            tx_clone.send_with_size(2, 80).unwrap(); // This should block
            send_completed_clone.store(true, Ordering::SeqCst);
        });

        // Give the thread time to start and block
        std::thread::sleep(Duration::from_millis(50));

        // The send should not have completed yet
        assert!(!send_completed.load(Ordering::SeqCst));

        // Now receive a message to free up space
        let _ = rx.recv().unwrap();

        // Wait for the send to complete
        handle.join().unwrap();
        assert!(send_completed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_select() {
        let (tx1, rx1) = channel::<i32>("chan1".to_owned(), 1000);
        let (tx2, rx2) = channel::<String>("chan2".to_owned(), 1000);

        // Send to first channel
        tx1.send_with_size(42, 8).unwrap();

        // Select should return from first channel
        crate::select! {
            recv(rx1) -> res => {
                assert_eq!(res.unwrap(), 42);
            },
            recv(rx2) -> _res => {
                panic!("expected rx1, got rx2");
            },
        }

        // Byte accounting should be updated
        assert_eq!(rx1.current_bytes(), 0);

        // Now send to second channel
        tx2.send_with_size("hello".to_owned(), 100).unwrap();

        // Select should return from second channel
        crate::select! {
            recv(rx1) -> _res => {
                panic!("expected rx2, got rx1");
            },
            recv(rx2) -> res => {
                assert_eq!(res.unwrap(), "hello");
            },
        }

        // Byte accounting should be updated
        assert_eq!(rx2.current_bytes(), 0);
    }
}
