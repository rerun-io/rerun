//! Async broadcast channel with byte-based backpressure.
//!
//! This wraps a [`tokio::sync::broadcast`] channel and adds byte-based capacity limits.
//! When the byte budget is exceeded, the sender will wait until space is available,
//! instead of dropping messages like a standard broadcast channel does.
//!
//! # Backpressure
//!
//! Backpressure is applied in two ways:
//! 1. **Byte limit**: When total bytes in flight exceeds `max_bytes`
//! 2. **Message count limit**: When approaching the underlying broadcast channel's capacity
//!
//! Each message sent through the channel is internally wrapped to track when all
//! receivers have consumed it. The byte count is only decremented when the last receiver
//! drops its copy of the message, ensuring correct accounting even with multiple receivers.

use std::fmt;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::{Notify, broadcast};

use re_byte_size::SizeBytes;

use crate::{BLOCKED_WARNING_THRESHOLD, BLOCKED_WARNING_THRESHOLD_SECS};

/// Reason why backpressure was applied.
enum BackpressureReason {
    /// The byte limit was exceeded.
    ByteLimit { max: u64 },

    /// The message count limit was exceeded.
    MessageCount { max: usize },
}

impl fmt::Display for BackpressureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLimit { max } => {
                write!(
                    f,
                    "byte limit of {} exceeded",
                    re_format::format_bytes(*max as f64)
                )
            }
            Self::MessageCount { max } => {
                write!(f, "message count limit of {max} exceeded")
            }
        }
    }
}

/// A broadcast channel that tracks bytes in flight and applies back-pressure.
///
/// When the total bytes currently in flight exceeds `max_bytes`, or when the
/// message count approaches capacity, the `send` method will wait until
/// receivers have consumed enough data.
pub struct Sender<T> {
    inner: broadcast::Sender<TrackedMessage<T>>,
    state: Arc<ChannelState>,
    max_bytes: u64,

    /// Maximum number of messages before we start applying backpressure.
    /// This is set slightly below the broadcast channel's capacity to avoid dropping.
    max_messages: usize,
}

struct ChannelState {
    /// Debug name for logging.
    debug_name: String,

    /// Protected mutable state.
    locked: Mutex<Locked>,

    /// Notified when state changes (bytes/messages freed up or all receivers dropped).
    state_changed: Notify,
}

struct Locked {
    /// Total bytes currently in flight.
    bytes_in_flight: u64,

    /// Number of messages currently in flight.
    messages_in_flight: usize,

    /// Number of active receivers.
    num_receivers: usize,
}

/// Internal wrapper around a message that decrements [`Locked::bytes_in_flight`] when dropped.
///
/// Multiple receivers will each get their own clone of this wrapper.
/// The bytes are only freed when ALL clones are dropped.
struct TrackedMessage<T> {
    value: T,
    state: Arc<TrackedInner>,
}

struct TrackedInner {
    size_bytes: u64,
    state: Arc<ChannelState>,
}

impl Drop for TrackedInner {
    fn drop(&mut self) {
        // Free up the bytes and message count
        {
            let mut locked = self.state.locked.lock();
            locked.bytes_in_flight -= self.size_bytes;
            locked.messages_in_flight -= 1;
        }
        self.state.state_changed.notify_waiters();
    }
}

impl<T: Clone> Clone for TrackedMessage<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> TrackedMessage<T> {
    /// Extract the value, consuming the wrapper.
    ///
    /// The `TrackedInner` (via the Arc) will decrement counters when all
    /// clones of this message have been dropped.
    fn into_value(self) -> T {
        self.value
    }
}

/// Result of trying to send a message.
enum TrySendResult<T> {
    /// Message was sent successfully to this many receivers.
    Sent(usize),

    /// Backpressure applied - channel is full, caller should retry.
    Full(T),

    /// No receivers - channel is disconnected.
    NoReceivers(T),
}

impl<T: Clone + SizeBytes> Sender<T> {
    /// Try to send a message without waiting.
    fn try_send(&self, value: T) -> TrySendResult<T> {
        let msg_bytes = value.total_size_bytes();
        let max_bytes = self.max_bytes;
        let max_messages = self.max_messages;

        {
            let mut locked = self.state.locked.lock();

            // Check if all receivers have been dropped
            if locked.num_receivers == 0 {
                return TrySendResult::NoReceivers(value);
            }

            let new_bytes = locked.bytes_in_flight + msg_bytes;
            let new_messages = locked.messages_in_flight + 1;

            // Allow if within limits, or if this is the first message (even if oversized)
            let is_first_message = locked.bytes_in_flight == 0 && locked.messages_in_flight == 0;
            if (new_messages <= max_messages && new_bytes <= max_bytes) || is_first_message {
                // Reserve the space
                locked.bytes_in_flight = new_bytes;
                locked.messages_in_flight = new_messages;
            } else {
                // Backpressure - log why we're blocked
                let debug_name = &self.state.debug_name;
                if max_messages < new_messages {
                    re_log::debug_once!(
                        "{debug_name}: Backpressure applied: {}",
                        BackpressureReason::MessageCount { max: max_messages }
                    );
                } else {
                    re_log::debug_once!(
                        "{debug_name}: Backpressure applied: {}",
                        BackpressureReason::ByteLimit { max: max_bytes }
                    );
                }
                return TrySendResult::Full(value);
            }
        }

        // Wrap the value in a TrackedMessage - the Arc<TrackedInner> will decrement counters when dropped
        let tracked = TrackedMessage {
            value,
            state: Arc::new(TrackedInner {
                size_bytes: msg_bytes,
                state: Arc::clone(&self.state),
            }),
        };

        // Send the message
        match self.inner.send(tracked) {
            Ok(n) => TrySendResult::Sent(n),
            Err(broadcast::error::SendError(tracked)) => {
                // No receivers - extract the value and let TrackedMessage drop
                // (which will decrement bytes_in_flight and messages_in_flight)
                TrySendResult::NoReceivers(tracked.into_value())
            }
        }
    }

    /// Send a message asynchronously, waiting if bytes or message count exceed limits.
    ///
    /// This will wait until there's enough space before sending.
    ///
    /// Returns `Err` if there are no receivers.
    pub async fn send_async(&self, value: T) -> Result<usize, NoReceivers<T>> {
        use std::time::Instant;

        let debug_name = &self.state.debug_name;
        let start = Instant::now();
        let mut warned = false;
        let mut value = value;

        loop {
            match self.try_send(value) {
                TrySendResult::Sent(n) => return Ok(n),
                TrySendResult::NoReceivers(v) => return Err(NoReceivers(v)),
                TrySendResult::Full(v) => value = v,
            }

            if !warned && BLOCKED_WARNING_THRESHOLD < start.elapsed() {
                re_log::warn!(
                    "{debug_name}: Sender has been blocked for over {BLOCKED_WARNING_THRESHOLD_SECS} seconds waiting for space in channel"
                );
                warned = true;
            }

            // Wait for notification that space is available or receivers changed:
            if warned {
                self.state.state_changed.notified().await;
            } else {
                // Wait with timeout so we can check elapsed time
                tokio::select! {
                    _ = self.state.state_changed.notified() => {}
                    _ = tokio::time::sleep(BLOCKED_WARNING_THRESHOLD) => {}
                }
            }
        }
    }

    /// Send a message, blocking the current thread if bytes or message count exceed limits.
    ///
    /// This will block until there's enough space before sending.
    ///
    /// Returns `Err` if there are no receivers.
    ///
    /// # Panics
    ///
    /// Panics if called from outside a tokio runtime context.
    pub fn send_blocking(&self, value: T) -> Result<usize, NoReceivers<T>> {
        tokio::runtime::Handle::current().block_on(self.send_async(value))
    }

    /// Subscribe to this channel.
    ///
    /// The new receiver will only receive messages sent AFTER this call.
    pub fn subscribe(&self) -> Receiver<T> {
        self.state.locked.lock().num_receivers += 1;
        Receiver {
            inner: self.inner.subscribe(),
            state: Arc::clone(&self.state),
        }
    }

    /// Returns the current number of bytes in flight.
    pub fn bytes_in_flight(&self) -> u64 {
        self.state.locked.lock().bytes_in_flight
    }

    /// Returns the current number of messages in flight.
    pub fn messages_in_flight(&self) -> usize {
        self.state.locked.lock().messages_in_flight
    }

    /// Returns the maximum bytes allowed in flight.
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// Returns the maximum messages allowed in flight before backpressure is applied.
    pub fn max_messages(&self) -> usize {
        self.max_messages
    }

    /// Returns the number of active receivers.
    pub fn receiver_count(&self) -> usize {
        self.state.locked.lock().num_receivers
    }
}

/// Error returned when sending fails because there are no receivers.
#[derive(Debug)]
pub struct NoReceivers<T>(pub T);

impl<T> std::fmt::Display for NoReceivers<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "No receivers")
    }
}

impl<T: std::fmt::Debug> std::error::Error for NoReceivers<T> {}

/// Error returned when receiving fails.
///
/// Unlike [`tokio::sync::broadcast::error::RecvError`], this does not include a `Lagged` variant
/// because our backpressure mechanism prevents message dropping due to capacity limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecvError {
    /// The channel has been closed (all senders have been dropped).
    Closed,
}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => write!(f, "Channel closed"),
        }
    }
}

impl std::error::Error for RecvError {}

/// A receiver for the back-pressure channel.
///
/// Messages are tracked internally; when all receivers have received and dropped
/// their copies, the bytes in flight counter is decremented, potentially unblocking
/// waiting senders.
pub struct Receiver<T> {
    inner: broadcast::Receiver<TrackedMessage<T>>,
    state: Arc<ChannelState>,
}

impl<T: Clone + SizeBytes> Receiver<T> {
    /// Receive the next message.
    ///
    /// # Errors
    ///
    /// Returns [`RecvError::Closed`] if all senders have been dropped.
    pub async fn recv(&mut self) -> Result<T, RecvError> {
        loop {
            match self.inner.recv().await {
                Ok(tracked) => {
                    return Ok(tracked.value);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(RecvError::Closed);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // This should never happen because our backpressure mechanism
                    // prevents the underlying broadcast channel from overflowing.
                    // If it does happen, it's a bug in our implementation.
                    re_log::error_once!(
                        "BUG: Bug in re_quota_channel. Data was LOST! Please report this issue."
                    );
                    // Try to recover by receiving the next message
                }
            }
        }
    }

    /// Returns the current number of bytes in flight.
    pub fn bytes_in_flight(&self) -> u64 {
        self.state.locked.lock().bytes_in_flight
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let num_receivers_left = {
            let mut locked = self.state.locked.lock();
            locked.num_receivers -= 1;
            locked.num_receivers
        };
        if num_receivers_left == 0 {
            // This was the last receiver, wake up any blocked senders
            self.state.state_changed.notify_waiters();
        }
    }
}

/// Create a new async byte-bounded broadcast channel.
///
/// # Arguments
///
/// * `debug_name` - A name used for logging messages about this channel.
/// * `max_messages` - Maximum number of messages the underlying broadcast channel can hold.
/// * `max_bytes` - Maximum bytes in flight before `send` waits.
///
/// # Backpressure
///
/// The channel applies backpressure in two ways:
/// 1. When total bytes in flight exceeds `max_bytes`
/// 2. When message count reaches `max_messages`
pub fn channel<T: Clone>(
    debug_name: impl Into<String>,
    max_messages: usize,
    max_bytes: u64,
) -> (Sender<T>, Receiver<T>) {
    let debug_name = debug_name.into();
    let max_messages = max_messages.max(1); // Ensure we allow at least 1 message

    if max_bytes == 0 {
        re_log::debug_warn_once!(
            "Channel '{debug_name}' has a memory limit of 0 bytes. Consider giving it at least a few MiB so that it can handle a quick burst of messages without blocking."
        );
    }

    let (inner, inner_rx) = broadcast::channel(max_messages);

    let state = Arc::new(ChannelState {
        debug_name,
        locked: Mutex::new(Locked {
            bytes_in_flight: 0,
            messages_in_flight: 0,
            num_receivers: 1, // Start with 1 for the initial receiver
        }),
        state_changed: Notify::new(),
    });

    (
        Sender {
            inner,
            state: Arc::clone(&state),
            max_bytes,
            max_messages,
        },
        Receiver {
            inner: inner_rx,
            state,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Clone, Debug, PartialEq)]
    struct TestMessage {
        data: Vec<u8>,
    }

    impl SizeBytes for TestMessage {
        fn heap_size_bytes(&self) -> u64 {
            self.data.len() as u64
        }
    }

    impl TestMessage {
        fn new(heap_bytes: usize) -> Self {
            Self {
                data: vec![0u8; heap_bytes],
            }
        }

        /// Returns expected `total_size_bytes` for this message
        fn expected_size(&self) -> u64 {
            self.total_size_bytes()
        }
    }

    #[tokio::test]
    async fn test_basic_send_recv() {
        let (tx, mut rx) = channel::<TestMessage>("test", 16, 1024);

        let msg = TestMessage::new(100);
        tx.send_async(msg.clone()).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_byte_tracking() {
        let (tx, mut rx) = channel::<TestMessage>("test", 16, 10_000);

        assert_eq!(tx.bytes_in_flight(), 0);

        let msg1 = TestMessage::new(100);
        let msg1_size = msg1.expected_size();
        tx.send_async(msg1).await.unwrap();
        assert_eq!(tx.bytes_in_flight(), msg1_size);

        let msg2 = TestMessage::new(200);
        let msg2_size = msg2.expected_size();
        tx.send_async(msg2).await.unwrap();
        assert_eq!(tx.bytes_in_flight(), msg1_size + msg2_size);

        // Receiving frees the bytes immediately (tracking is internal)
        let _received = rx.recv().await.unwrap();
        assert_eq!(rx.bytes_in_flight(), msg2_size);

        let _received = rx.recv().await.unwrap();
        assert_eq!(rx.bytes_in_flight(), 0);
    }

    #[tokio::test]
    async fn test_blocking_backpressure() {
        // Use a limit that accounts for total_size_bytes (stack + heap)
        let msg = TestMessage::new(100);
        let msg_size = msg.expected_size();
        let max_bytes = msg_size * 2; // Allow exactly 2 messages

        let (tx, mut rx) = channel::<TestMessage>("test", 16, max_bytes);
        let tx = Arc::new(tx);

        // Send first message
        tx.send_async(TestMessage::new(100)).await.unwrap();

        // Send second message (now at limit)
        tx.send_async(TestMessage::new(100)).await.unwrap();

        assert_eq!(tx.bytes_in_flight(), msg_size * 2);

        // This send should block because we're at the limit
        let tx_clone = Arc::clone(&tx);
        let send_handle = tokio::spawn(async move {
            tx_clone.send_async(TestMessage::new(100)).await.unwrap();
        });

        // Give the sender a chance to block
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!send_handle.is_finished());

        // Receive one message and drop it to free up space
        let received = rx.recv().await.unwrap();
        drop(received);

        // Now the sender should complete
        tokio::time::timeout(Duration::from_millis(100), send_handle)
            .await
            .expect("send should complete after receiving")
            .unwrap();
    }

    #[tokio::test]
    async fn test_oversized_message_when_empty() {
        let small_limit = 10; // Smaller than a TestMessage
        let (tx, _rx) = channel::<TestMessage>("test", 16, small_limit);

        // Even though this message is larger than max_bytes,
        // it should be allowed because the channel is empty
        let large_msg = TestMessage::new(200);
        let large_msg_size = large_msg.expected_size();
        tx.send_async(large_msg).await.unwrap();

        assert_eq!(tx.bytes_in_flight(), large_msg_size);
    }

    #[tokio::test]
    async fn test_send_returns_error_when_receiver_dropped() {
        // Use a limit that will cause blocking
        let msg = TestMessage::new(80);
        let msg_size = msg.expected_size();
        let max_bytes = msg_size; // Only allow one message

        let (tx, rx) = channel::<TestMessage>("test", 16, max_bytes);
        let tx = Arc::new(tx);

        // Fill up the channel
        tx.send_async(TestMessage::new(80)).await.unwrap();

        // Spawn a task that will try to send another message (which should block)
        let tx_clone = Arc::clone(&tx);
        let send_handle = tokio::spawn(async move {
            // This should block waiting for space, then return Err when receiver is dropped
            tx_clone.send_async(TestMessage::new(80)).await
        });

        // Give the task time to start and block
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The send should not have completed yet (it's blocked waiting for space)
        assert!(!send_handle.is_finished());

        // Drop the receiver - this should cause the blocked send to return
        drop(rx);

        // Wait for the send to complete and check the result
        let result = tokio::time::timeout(Duration::from_millis(100), send_handle)
            .await
            .expect("send should complete after receiver dropped")
            .unwrap();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_receivers() {
        let (tx, mut rx1) = channel::<TestMessage>("test", 16, 10_000);
        let mut rx2 = tx.subscribe();

        assert_eq!(tx.receiver_count(), 2);

        let msg = TestMessage::new(100);
        tx.send_async(msg.clone()).await.unwrap();

        // Both receivers should get the message
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert_eq!(received1, msg);
        assert_eq!(received2, msg);
    }

    #[tokio::test]
    async fn test_multiple_receivers_byte_tracking() {
        let (tx, mut rx1) = channel::<TestMessage>("test", 16, 10_000);
        let mut rx2 = tx.subscribe();

        let msg = TestMessage::new(100);
        let msg_size = msg.expected_size();
        tx.send_async(msg).await.unwrap();

        assert_eq!(tx.bytes_in_flight(), msg_size);

        // First receiver gets it - bytes still in flight (broadcast clones internally)
        let _received1 = rx1.recv().await.unwrap();
        assert_eq!(tx.bytes_in_flight(), msg_size);

        // Second receiver gets it - bytes freed after both have received
        let _received2 = rx2.recv().await.unwrap();
        assert_eq!(tx.bytes_in_flight(), 0);
    }

    #[tokio::test]
    async fn test_receiver_count_tracking() {
        let (tx, rx1) = channel::<TestMessage>("test", 16, 10_000);
        assert_eq!(tx.receiver_count(), 1);

        let rx2 = tx.subscribe();
        assert_eq!(tx.receiver_count(), 2);

        let rx3 = tx.subscribe();
        assert_eq!(tx.receiver_count(), 3);

        drop(rx1);
        assert_eq!(tx.receiver_count(), 2);

        drop(rx2);
        assert_eq!(tx.receiver_count(), 1);

        drop(rx3);
        assert_eq!(tx.receiver_count(), 0);
    }

    #[tokio::test]
    async fn test_send_fails_immediately_with_no_receivers() {
        let (tx, rx) = channel::<TestMessage>("test", 16, 10_000);
        drop(rx);

        let result = tx.send_async(TestMessage::new(100)).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_message_count_backpressure() {
        // Create a channel with max_messages = 3
        let (tx, mut rx) = channel::<TestMessage>("test", 3, u64::MAX); // Large byte limit, we're testing message count
        let tx = Arc::new(tx);

        assert_eq!(tx.max_messages(), 3);
        assert_eq!(tx.messages_in_flight(), 0);

        // Send first message
        tx.send_async(TestMessage::new(10)).await.unwrap();
        assert_eq!(tx.messages_in_flight(), 1);

        // Send second message
        tx.send_async(TestMessage::new(10)).await.unwrap();
        assert_eq!(tx.messages_in_flight(), 2);

        // Send third message (now at message limit)
        tx.send_async(TestMessage::new(10)).await.unwrap();
        assert_eq!(tx.messages_in_flight(), 3);

        // This send should block because we're at the message count limit
        let tx_clone = Arc::clone(&tx);
        let send_handle = tokio::spawn(async move {
            tx_clone.send_async(TestMessage::new(10)).await.unwrap();
        });

        // Give the sender a chance to block
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!send_handle.is_finished());

        // Receive one message to free up space
        let _received = rx.recv().await.unwrap();

        // Now the sender should complete
        tokio::time::timeout(Duration::from_millis(100), send_handle)
            .await
            .expect("send should complete after receiving")
            .unwrap();

        assert_eq!(tx.messages_in_flight(), 3);
    }

    #[tokio::test]
    async fn test_message_count_tracking() {
        let (tx, mut rx) = channel::<TestMessage>("test", 16, 10_000);

        assert_eq!(tx.messages_in_flight(), 0);

        tx.send_async(TestMessage::new(100)).await.unwrap();
        assert_eq!(tx.messages_in_flight(), 1);

        tx.send_async(TestMessage::new(100)).await.unwrap();
        assert_eq!(tx.messages_in_flight(), 2);

        tx.send_async(TestMessage::new(100)).await.unwrap();
        assert_eq!(tx.messages_in_flight(), 3);

        // Receive messages - count decrements immediately on recv
        let _msg = rx.recv().await.unwrap();
        assert_eq!(tx.messages_in_flight(), 2);

        let _msg = rx.recv().await.unwrap();
        assert_eq!(tx.messages_in_flight(), 1);

        let _msg = rx.recv().await.unwrap();
        assert_eq!(tx.messages_in_flight(), 0);
    }
}
