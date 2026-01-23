//! Synchronous mpsc channel with byte-based backpressure.
//!
//! This wraps a [`crossbeam::channel`] and adds byte-based capacity limits.
//! When the byte budget is exceeded, the sender will block until space is available.
//!
//! On `wasm32`, blocking is not allowed, so we only log a warning when the budget is exceeded.

use std::sync::Arc;

use parking_lot::{FairMutex, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use parking_lot::Condvar;

pub use crossbeam::channel::{RecvError, RecvTimeoutError, SendError, TryRecvError};

mod select;
pub use select::{Select, SelectTimeoutError, SelectedOperation, TrySelectError};

pub use crate::SizedMessage;
pub use crate::try_send_error::TrySendError;

struct SharedState {
    /// Debug name for logging.
    debug_name: String,

    /// Total capacity in bytes. Never changes.
    capacity_bytes: u64,

    /// Whomever holds this is allowed to send (if there is space available).
    ///
    /// Used to ensure fair access to the channel among multiple senders,
    /// preventing starvation.
    active_sender: FairMutex<()>,

    /// Protected by mutex for use with condvar.
    bytes_in_flight: Mutex<u64>,

    /// Signaled when bytes decrease (i.e., when a message is received).
    #[cfg(not(target_arch = "wasm32"))]
    less_bytes_in_flight: Condvar,
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

    // Blocking version for native
    #[cfg(not(target_arch = "wasm32"))]
    fn send_impl(&self, msg: T, size_bytes: u64) -> Result<(), SendError<T>> {
        let _sender_lock = self.shared.active_sender.lock();

        let capacity = self.shared.capacity_bytes;

        if size_bytes <= capacity {
            // Normal case: wait until we have room

            {
                let mut bytes_in_flight = self.shared.bytes_in_flight.lock();

                if capacity < *bytes_in_flight + size_bytes {
                    re_log::debug_once!(
                        "{}: Channel byte budget ({}) exceeded. Blocking until space is available…",
                        self.shared.debug_name,
                        re_format::format_bytes(capacity as f64),
                    );
                    while capacity < *bytes_in_flight + size_bytes {
                        self.shared.less_bytes_in_flight.wait(&mut bytes_in_flight);
                    }
                }

                *bytes_in_flight += size_bytes;
            }

            self.tx
                .send(SizedMessage { msg, size_bytes })
                .map_err(|err| SendError(err.0.msg))
        } else {
            // Special case: message is larger than total capacity
            re_log::debug_once!(
                "{}: Message size ({}) exceeds channel capacity ({}). \
                 Waiting for channel to empty before sending.",
                self.shared.debug_name,
                re_format::format_bytes(size_bytes as f64),
                re_format::format_bytes(capacity as f64),
            );

            {
                // Wait until the channel is completely empty
                let mut bytes_in_flight = self.shared.bytes_in_flight.lock();
                while 0 < *bytes_in_flight {
                    self.shared.less_bytes_in_flight.wait(&mut bytes_in_flight);
                }

                // Now send (we'll temporarily exceed capacity, but that's expected)
                *bytes_in_flight += size_bytes;
            }

            self.tx
                .send(SizedMessage { msg, size_bytes })
                .map_err(|err| SendError(err.0.msg))
        }
    }

    // Non-blocking version for web
    #[cfg(target_arch = "wasm32")]
    fn send_impl(&self, msg: T, size_bytes: u64) -> Result<(), SendError<T>> {
        let capacity = self.shared.capacity_bytes;

        {
            let mut current = self.shared.bytes_in_flight.lock();
            let new_total = *current + size_bytes;

            if capacity < new_total {
                re_log::debug_once!(
                    "{}: Channel byte budget ({}) exceeded. \
                    Cannot block on web; sending anyway.",
                    self.shared.debug_name,
                    re_format::format_bytes(capacity as f64),
                );
            }

            *current = new_total;
        }

        self.tx
            .send(SizedMessage { msg, size_bytes })
            .map_err(|err| SendError(err.0.msg))
    }

    /// Try to send a message without blocking.
    ///
    /// Returns an error if the channel is full (by byte count) or disconnected.
    pub fn try_send(&self, msg: T, size_bytes: u64) -> Result<(), TrySendError<T>> {
        let _sender_lock = self.shared.active_sender.lock();

        let capacity = self.shared.capacity_bytes;

        {
            let mut current = self.shared.bytes_in_flight.lock();

            // Check if we have room (allow oversized messages if channel is empty)
            if capacity < *current + size_bytes && 0 < *current {
                return Err(TrySendError::Full(msg));
            }

            *current += size_bytes;
        }

        self.tx
            .send(SizedMessage { msg, size_bytes })
            .map_err(|SendError(SizedMessage { msg, .. })| TrySendError::Disconnected(msg))
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
        *self.shared.bytes_in_flight.lock()
    }

    /// Returns the byte capacity of the channel.
    #[inline]
    pub fn capacity_bytes(&self) -> u64 {
        self.shared.capacity_bytes
    }
}

// ----------------------------------------------------------------------------

/// The receiving end of a byte-bounded channel.
#[derive(Clone)]
pub struct Receiver<T> {
    rx: crossbeam::channel::Receiver<SizedMessage<T>>,
    shared: Arc<SharedState>,
}

impl<T> Receiver<T> {
    /// Receive a message, blocking until one is available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn recv(&self) -> Result<T, RecvError> {
        let sized_msg = self.rx.recv()?;
        self.manual_on_receive(sized_msg.size_bytes);
        Ok(sized_msg.msg)
    }

    /// Receive a message with a timeout.
    #[cfg(not(target_arch = "wasm32"))]
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
        *self.shared.bytes_in_flight.lock()
    }

    /// Returns the byte capacity of the channel.
    #[inline]
    pub fn capacity_bytes(&self) -> u64 {
        self.shared.capacity_bytes
    }

    /// Create an iterator that receives messages.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.recv().ok())
    }

    /// Access the inner crossbeam receiver.
    ///
    /// This can be useful for use with `crossbeam::select!`.
    ///
    /// WARNING: if you receive a message directly from this receiver,
    /// you must manually call [`Self::manual_on_receive`] to update the byte count!
    pub fn inner(&self) -> &crossbeam::channel::Receiver<SizedMessage<T>> {
        &self.rx
    }

    /// For use with [`Self::inner`]: notify the channel that a message of the given size
    /// has been received, so that the byte count can be updated.
    pub fn manual_on_receive(&self, size_bytes: u64) {
        {
            let mut current = self.shared.bytes_in_flight.lock();
            *current = current.saturating_sub(size_bytes);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.shared.less_bytes_in_flight.notify_all();
        }
    }

    /// A non-blocking iterator over messages in the channel.
    ///
    /// Each call to next returns a message if there is one ready to be received. The iterator never blocks waiting for the next message.
    pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.try_recv().ok())
    }
}

// ----------------------------------------------------------------------------

/// Create a new byte-bounded channel.
///
/// # Caveats
/// Sending has significant overhead compared to a normal bounded channel.
///
/// It optimized for few senders sending large messages,
///
/// # Arguments
///
/// * `debug_name` - A name used for logging messages about this channel.
/// * `capacity_bytes` - The maximum number of bytes allowed in the channel before
///   backpressure is applied. On native platforms, [`Sender::send`] will block
///   when this limit is reached. On `wasm32`, a warning is logged but sending continues.
pub fn channel<T>(debug_name: impl Into<String>, capacity_bytes: u64) -> (Sender<T>, Receiver<T>) {
    #[expect(clippy::disallowed_methods)] // This crate adds its own byte-based bound/backpressure.
    let (tx, rx) = crossbeam::channel::unbounded();

    let shared = Arc::new(SharedState {
        debug_name: debug_name.into(),

        capacity_bytes,

        active_sender: FairMutex::new(()),

        bytes_in_flight: Mutex::new(0),

        #[cfg(not(target_arch = "wasm32"))]
        less_bytes_in_flight: Condvar::new(),
    });

    let sender = Sender {
        tx,
        shared: Arc::clone(&shared),
    };

    let receiver = Receiver { rx, shared };

    (sender, receiver)
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
    fn test_try_iter() {
        let (tx, rx) = channel::<u32>("test".to_owned(), 1000);

        // Send some messages
        tx.send_with_size(1, 10).unwrap();
        tx.send_with_size(2, 10).unwrap();
        tx.send_with_size(3, 10).unwrap();

        // Collect all messages using try_iter
        let messages: Vec<u32> = rx.try_iter().collect();
        assert_eq!(messages, vec![1, 2, 3]);

        // Channel should be empty now
        assert!(rx.is_empty());
        assert_eq!(rx.current_bytes(), 0);

        // try_iter on empty channel should yield no items
        let messages: Vec<u32> = rx.try_iter().collect();
        assert_eq!(messages, Vec::<u32>::new());
    }
}
