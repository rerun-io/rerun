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

    /// Drain all buffered [`LogMsg`]es and return them.
    ///
    /// Only applies to sinks that maintain a backlog.
    #[inline]
    fn drain_backlog(&self) -> Vec<LogMsg> {
        vec![]
    }

    /// Blocks until all pending data in the sink's send buffers has been fully flushed.
    ///
    /// See also [`LogSink::drop_if_disconnected`].
    fn flush_blocking(&self);

    /// Drops all pending data currently sitting in the sink's send buffers if it is unable to
    /// flush it for any reason (e.g. a broken TCP connection for a [`TcpSink`]).
    #[inline]
    fn drop_if_disconnected(&self) {}
}

// ----------------------------------------------------------------------------

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

    #[inline]
    fn flush_blocking(&self) {}
}

/// Store log messages directly in memory.
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

    #[inline]
    fn flush_blocking(&self) {}
}

/// The storage used by [`MemorySink`].
#[derive(Default, Clone)]
pub struct MemorySinkStorage(Arc<RwLock<Vec<LogMsg>>>);

impl MemorySinkStorage {
    /// Write access to the inner array of [`LogMsg`].
    #[inline]
    fn write(&self) -> parking_lot::RwLockWriteGuard<'_, Vec<LogMsg>> {
        self.0.write()
    }

    /// Read access to the inner array of [`LogMsg`].
    #[inline]
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Vec<LogMsg>> {
        self.0.read()
    }

    /// Consumes and returns the inner array of [`LogMsg`].
    #[inline]
    pub fn take(&self) -> Vec<LogMsg> {
        std::mem::take(&mut *self.0.write())
    }
}

impl MemorySinkStorage {
    /// Convert the stored messages into an in-memory Rerun log file.
    #[inline]
    pub fn concat_memory_sinks_as_bytes(
        sinks: &[&Self],
    ) -> Result<Vec<u8>, re_log_encoding::encoder::EncodeError> {
        let mut buffer = std::io::Cursor::new(Vec::new());

        {
            let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
            let mut encoder =
                re_log_encoding::encoder::Encoder::new(encoding_options, &mut buffer)?;
            for sink in sinks {
                for message in sink.read().iter() {
                    encoder.append(message)?;
                }
            }
            encoder.finish()?;
        }

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
    fn flush_blocking(&self) {
        self.client.flush();
    }

    #[inline]
    fn drop_if_disconnected(&self) {
        self.client.drop_if_disconnected();
    }
}
