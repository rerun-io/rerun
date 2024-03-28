use std::fmt;
use std::sync::Arc;

use parking_lot::RwLock;
use re_log_types::{BlueprintActivationCommand, LogMsg, StoreId};

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

    /// Send a blueprint directly to the log-sink.
    ///
    /// This mirrors the behavior of [`crate::RecordingStream::send_blueprint`].
    fn send_blueprint(&self, blueprint: Vec<LogMsg>, activation_cmd: BlueprintActivationCommand) {
        let mut blueprint_id = None;
        for msg in blueprint {
            if blueprint_id.is_none() {
                blueprint_id = Some(msg.store_id().clone());
            }
            self.send(msg);
        }

        if let Some(blueprint_id) = blueprint_id {
            if blueprint_id == activation_cmd.blueprint_id {
                // Let the viewer know that the blueprint has been fully received,
                // and that it can now be activated.
                // We don't want to activate half-loaded blueprints, because that can be confusing,
                // and can also lead to problems with space-view heuristics.
                self.send(activation_cmd.into());
            } else {
                re_log::warn!(
                    "Blueprint ID mismatch when sending blueprint: {} != {}. Ignoring activation.",
                    blueprint_id,
                    activation_cmd.blueprint_id
                );
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Store log messages in memory until you call [`LogSink::drain_backlog`].
#[derive(Default)]
pub struct BufferedSink(parking_lot::Mutex<Vec<LogMsg>>);

impl Drop for BufferedSink {
    fn drop(&mut self) {
        for msg in self.0.lock().iter() {
            // Sinks intentionally end up with pending SetStoreInfo messages
            // these are fine to drop safely. Anything else should produce a
            // warning.
            if !matches!(msg, LogMsg::SetStoreInfo(_)) {
                re_log::warn!("Dropping data in BufferedSink");
                return;
            }
        }
    }
}

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

impl fmt::Debug for BufferedSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BufferedSink {{ {} messages }}", self.0.lock().len())
    }
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

impl fmt::Debug for MemorySink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemorySink {{ {} messages }}",
            self.buffer().read().len()
        )
    }
}

/// The storage used by [`MemorySink`].
#[derive(Default, Clone)]
pub struct MemorySinkStorage {
    msgs: Arc<RwLock<Vec<LogMsg>>>,
    pub(crate) rec: Option<crate::RecordingStream>,
}

impl Drop for MemorySinkStorage {
    fn drop(&mut self) {
        for msg in self.msgs.read().iter() {
            // Sinks intentionally end up with pending SetStoreInfo messages
            // these are fine to drop safely. Anything else should produce a
            // warning.
            if !matches!(msg, LogMsg::SetStoreInfo(_)) {
                re_log::warn!("Dropping data in MemorySink");
                return;
            }
        }
    }
}

impl MemorySinkStorage {
    /// Write access to the inner array of [`LogMsg`].
    #[inline]
    fn write(&self) -> parking_lot::RwLockWriteGuard<'_, Vec<LogMsg>> {
        self.msgs.write()
    }

    /// Read access to the inner array of [`LogMsg`].
    #[inline]
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Vec<LogMsg>> {
        self.msgs.read()
    }

    /// How many messages are currently written to this memory sink
    #[inline]
    pub fn num_msgs(&self) -> usize {
        self.read().len()
    }

    /// Consumes and returns the inner array of [`LogMsg`].
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn take(&self) -> Vec<LogMsg> {
        if let Some(rec) = self.rec.as_ref() {
            // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
            // in this flush; it's just a matter of making the table batcher tick early.
            rec.flush_blocking();
        }
        std::mem::take(&mut *self.msgs.write())
    }

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
        }

        Ok(buffer.into_inner())
    }

    #[inline]
    /// Get the [`StoreId`] from the associated `RecordingStream` if it exists.
    pub fn store_id(&self) -> Option<StoreId> {
        self.rec
            .as_ref()
            .and_then(|rec| rec.store_info().map(|info| info.store_id.clone()))
    }
}
// ----------------------------------------------------------------------------

/// Stream log messages to a Rerun TCP server.
#[derive(Debug)]
pub struct TcpSink {
    client: re_sdk_comms::Client,
}

impl TcpSink {
    /// Connect to the given address in a background thread.
    /// Retries until successful.
    ///
    /// `flush_timeout` is the minimum time the [`TcpSink`] will wait during a flush
    /// before potentially dropping data. Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    #[inline]
    pub fn new(addr: std::net::SocketAddr, flush_timeout: Option<std::time::Duration>) -> Self {
        Self {
            client: re_sdk_comms::Client::new(addr, flush_timeout),
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
