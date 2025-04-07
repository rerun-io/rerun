use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use re_grpc_client::message_proxy::write::{Client as MessageProxyClient, Options};
use re_log_encoding::encoder::encode_as_bytes_local;
use re_log_encoding::encoder::{local_raw_encoder, EncodeError};
use re_log_types::{BlueprintActivationCommand, LogMsg, StoreId};

use crate::RecordingStream;

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
    /// If applicable, this should flush all data to any underlying OS-managed file descriptors.
    /// See also [`LogSink::drop_if_disconnected`].
    fn flush_blocking(&self);

    /// Drops all pending data currently sitting in the sink's send buffers if it is unable to
    /// flush it for any reason.
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
                // and can also lead to problems with view heuristics.
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
pub struct MemorySink(MemorySinkStorage);

impl MemorySink {
    /// Create a new [`MemorySink`] with an associated [`RecordingStream`].
    #[inline]
    pub fn new(rec: RecordingStream) -> Self {
        Self(MemorySinkStorage::new(rec))
    }

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

    #[inline]
    fn drain_backlog(&self) -> Vec<LogMsg> {
        // Note that When draining the backlog, we don't call `take` since that would flush
        // the stream. But drain_backlog is being called as part of `set_sink`, which has already queued
        // a flush of the batcher. Queueing a second flush here seems to lead to a deadlock
        // at shutdown.
        std::mem::take(&mut (self.0.write()))
    }
}

impl fmt::Debug for MemorySink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MemorySink {{ {} messages }}", self.buffer().num_msgs())
    }
}

#[derive(Default)]
struct MemorySinkStorageInner {
    msgs: Vec<LogMsg>,
    has_been_used: bool,
}

/// The storage used by [`MemorySink`].
#[derive(Clone)]
pub struct MemorySinkStorage {
    inner: Arc<Mutex<MemorySinkStorageInner>>,
    pub(crate) rec: RecordingStream,
}

impl Drop for MemorySinkStorage {
    fn drop(&mut self) {
        let inner = self.inner.lock();
        if !inner.has_been_used {
            for msg in &inner.msgs {
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
}

impl MemorySinkStorage {
    /// Create a new [`MemorySinkStorage`] with an associated [`RecordingStream`].
    fn new(rec: RecordingStream) -> Self {
        Self {
            inner: Default::default(),
            rec,
        }
    }

    /// Write access to the inner array of [`LogMsg`].
    #[inline]
    fn write(&self) -> parking_lot::MappedMutexGuard<'_, Vec<LogMsg>> {
        let mut inner = self.inner.lock();
        inner.has_been_used = false;
        parking_lot::MutexGuard::map(inner, |inner| &mut inner.msgs)
    }

    /// How many messages are currently written to this memory sink
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn num_msgs(&self) -> usize {
        // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
        // in this flush; it's just a matter of making the table batcher tick early.
        self.rec.flush_blocking();
        self.inner.lock().msgs.len()
    }

    /// Consumes and returns the inner array of [`LogMsg`].
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn take(&self) -> Vec<LogMsg> {
        // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
        // in this flush; it's just a matter of making the table batcher tick early.
        self.rec.flush_blocking();
        std::mem::take(&mut (self.write()))
    }

    /// Convert the stored messages into an in-memory Rerun log file.
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn concat_memory_sinks_as_bytes(sinks: &[&Self]) -> Result<Vec<u8>, EncodeError> {
        let mut encoder = local_raw_encoder()?;

        for sink in sinks {
            // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
            // in this flush; it's just a matter of making the table batcher tick early.
            sink.rec.flush_blocking();
            let mut inner = sink.inner.lock();
            inner.has_been_used = true;

            for message in &inner.msgs {
                encoder.append(message)?;
            }
        }

        encoder.finish()?;

        Ok(encoder.into_inner())
    }

    /// Drain the stored messages and return them as an in-memory RRD.
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn drain_as_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
        // in this flush; it's just a matter of making the table batcher tick early.
        self.rec.flush_blocking();

        let mut inner = self.inner.lock();
        inner.has_been_used = true;

        encode_as_bytes_local(std::mem::take(&mut inner.msgs).into_iter().map(Ok))
    }

    #[inline]
    /// Get the [`StoreId`] from the associated `RecordingStream` if it exists.
    pub fn store_id(&self) -> Option<StoreId> {
        self.rec.store_info().map(|info| info.store_id.clone())
    }
}
// ----------------------------------------------------------------------------

type LogMsgCallback = Box<dyn Fn(&[LogMsg]) + Send + Sync>;

/// A sink which forwards all log messages to a callback without any buffering.
pub struct CallbackSink {
    // We often receive only one element, so we use SmallVec to avoid heap allocation
    callback: LogMsgCallback,
}

impl CallbackSink {
    /// Create a new `CallbackSink` with the given callback function.
    #[inline]
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&[LogMsg]) + Send + Sync + 'static,
    {
        Self {
            callback: Box::new(callback),
        }
    }
}

impl LogSink for CallbackSink {
    #[inline]
    fn send(&self, msg: LogMsg) {
        (self.callback)(&[msg]);
    }

    #[inline]
    fn send_all(&self, messages: Vec<LogMsg>) {
        (self.callback)(&messages[..]);
    }

    #[inline]
    fn flush_blocking(&self) {}
}

// ----------------------------------------------------------------------------

/// Stream log messages to an a remote Rerun server.
pub struct GrpcSink {
    client: MessageProxyClient,
}

impl GrpcSink {
    /// Connect to the in-memory storage node over HTTP.
    ///
    /// `flush_timeout` is the minimum time the [`GrpcSink`] will wait during a flush
    /// before potentially dropping data. Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    ///
    /// ### Example
    ///
    /// ```ignore
    /// GrpcSink::new("rerun+http://127.0.0.1:9434/proxy");
    /// ```
    #[inline]
    pub fn new(uri: re_uri::ProxyUri, flush_timeout: Option<Duration>) -> Self {
        let options = Options {
            flush_timeout,
            ..Default::default()
        };
        Self {
            client: MessageProxyClient::new(uri, options),
        }
    }
}

impl LogSink for GrpcSink {
    fn send(&self, msg: LogMsg) {
        self.client.send_msg(msg);
    }

    fn flush_blocking(&self) {
        self.client.flush();
    }
}
