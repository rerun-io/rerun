use std::sync::Arc;

use parking_lot::RwLock;
use re_log_types::LogMsg;

/// Where the SDK sends its log messages.
pub trait LogSink: Send + Sync + 'static {
    /// Send this log message.
    fn send(&self, msg: LogMsg);

    /// Send all these log messages.
    #[inline]
    fn send_all(&self, messages: Vec<LogMsg>) {
        for msg in messages {
            self.send(msg);
        }
    }

    // TODO: this needs to go
    /// Drain all buffered [`LogMsg`]es and return them.
    #[inline]
    fn drain_backlog(&self) -> Vec<LogMsg> {
        vec![]
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    #[inline]
    fn flush(&self) {}

    // TODO: that's very leaky and weird?
    /// If the TCP session is disconnected, allow it to quit early and drop unsent messages.
    #[inline]
    fn drop_msgs_if_disconnected(&self) {}

    // TODO: this has to go
    /// Returns `false` if this sink just discards all messages.
    #[inline]
    fn is_enabled(&self) -> bool {
        true
    }
}

// ----------------------------------------------------------------------------

// TODO: this has to go

struct DisabledSink;

impl LogSink for DisabledSink {
    #[inline]
    fn send(&self, _msg: LogMsg) {
        // It's intended that the logging SDK should drop messages earlier than this if logging is disabled.
        re_log::debug_once!("Logging is disabled, dropping message(s).");
    }

    #[inline]
    fn is_enabled(&self) -> bool {
        false
    }
}

/// A sink that does nothing. All log messages are just dropped.
#[inline]
pub fn disabled() -> Box<dyn LogSink> {
    Box::new(DisabledSink)
}

// ----------------------------------------------------------------------------

// TODO: this has to go

/// Store log messages in memory until you call [`LogSink::drain_backlog`].
#[derive(Default)]
pub struct BufferedSink(parking_lot::Mutex<Vec<LogMsg>>);

impl BufferedSink {
    /// An empty buffer.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
}

impl LogSink for BufferedSink {
    #[inline]
    fn send(&self, msg: LogMsg) {
        self.0.lock().push(msg);
    }

    #[inline]
    fn send_all(&self, mut messages: Vec<LogMsg>) {
        self.0.lock().append(&mut messages);
    }

    #[inline]
    fn drain_backlog(&self) -> Vec<LogMsg> {
        std::mem::take(&mut self.0.lock())
    }
}

/// Store log messages directly in memory
///
/// Although very similar to `BufferedSink` this sink is a real-endpoint. When creating
/// a new sink the logged messages stay with the `MemorySink` (`drain_backlog` does nothing).
///
/// Additionally the raw storage can be accessed and used to create an in-memory RRD.
/// This is useful for things like the inline rrd-viewer in Jupyter notebooks.
#[derive(Default)]
pub struct MemorySink(MemorySinkStorage);

impl MemorySink {
    /// Access the raw `MemorySinkStorage`
    #[inline]
    pub fn buffer(&self) -> MemorySinkStorage {
        self.0.clone()
    }
}

impl LogSink for MemorySink {
    #[inline]
    fn send(&self, msg: LogMsg) {
        self.0.write().push(msg);
    }

    #[inline]
    fn send_all(&self, mut messages: Vec<LogMsg>) {
        self.0.write().append(&mut messages);
    }
}

/// The storage used by [`MemorySink`]
#[derive(Default, Clone)]
pub struct MemorySinkStorage(Arc<RwLock<Vec<LogMsg>>>);

impl MemorySinkStorage {
    /// Lock the contained buffer
    #[inline]
    fn write(&self) -> parking_lot::RwLockWriteGuard<'_, Vec<LogMsg>> {
        self.0.write()
    }

    // TODO
    #[inline]
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Vec<LogMsg>> {
        self.0.read()
    }

    // TODO
    #[inline]
    pub fn take(&self) -> Vec<LogMsg> {
        std::mem::take(&mut *self.0.write())
    }

    // TODO: that sounds like the greatest opportunity ever to do a perfect batch then?
    /// Convert the stored messages into an in-memory Rerun log file
    #[inline]
    pub fn rrd_as_bytes(&self) -> Result<Vec<u8>, re_log_encoding::encoder::EncodeError> {
        let messages = self.write();
        let mut buffer = std::io::Cursor::new(Vec::new());
        re_log_encoding::encoder::encode(messages.iter(), &mut buffer)?;
        Ok(buffer.into_inner())
    }
}

// ----------------------------------------------------------------------------

/// Stream log messages to a Rerun TCP server.
pub struct TcpSink {
    client: re_sdk_comms::Client,
}

impl TcpSink {
    /// Connect to the given address in a background thread.
    /// Retries until successful.
    #[inline]
    pub fn new(addr: std::net::SocketAddr) -> Self {
        Self {
            client: re_sdk_comms::Client::new(addr),
        }
    }
}

impl LogSink for TcpSink {
    #[inline]
    fn send(&self, msg: LogMsg) {
        self.client.send(msg);
    }

    #[inline]
    fn flush(&self) {
        self.client.flush();
    }

    #[inline]
    fn drop_msgs_if_disconnected(&self) {
        self.client.drop_if_disconnected();
    }
}
