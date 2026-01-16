use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use re_chunk::ChunkBatcherConfig;
use re_grpc_client::write::{Client as MessageProxyClient, GrpcFlushError, Options};
use re_log_encoding::{EncodeError, Encoder};
use re_log_types::{BlueprintActivationCommand, LogMsg, StoreId};

use crate::RecordingStream;

/// An error that can occur when flushing.
#[derive(Debug, thiserror::Error)]
pub enum SinkFlushError {
    /// Flush timed out - not all log messages were sent
    #[error("Flush timed out - not all log messages were sent")]
    Timeout,

    /// An error occurred before reaching the timeout.
    #[error("{message}")]
    Failed {
        /// Details
        message: String,
    },
}

impl SinkFlushError {
    /// Custom error occurred
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }
}

/// Where the SDK sends its log messages.
pub trait LogSink: Send + Sync + 'static + std::any::Any {
    /// Send this log message.
    ///
    /// This should optionally block in order to apply backpressure.
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
    /// Returns an error if the underlying sink is in a bad state,
    /// e.g. if a network connection has been severed, or failed to connect within a certain timeout.
    ///
    /// If applicable, this should flush all data to any underlying OS-managed file descriptors.
    fn flush_blocking(&self, timeout: Duration) -> Result<(), SinkFlushError>;

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
                    "Blueprint ID mismatch when sending blueprint: {:?} != {:?}. Ignoring activation.",
                    blueprint_id,
                    activation_cmd.blueprint_id
                );
            }
        }
    }

    /// The default batcher configuration used for new (!) [`RecordingStream`]s with this sink.
    fn default_batcher_config(&self) -> ChunkBatcherConfig {
        ChunkBatcherConfig::DEFAULT
    }
}

// ----------------------------------------------------------------------------

/// Stream to multiple sinks at the same time.
pub struct MultiSink(parking_lot::Mutex<Vec<Box<dyn LogSink>>>);

impl MultiSink {
    /// Combine multiple sinks into one.
    ///
    /// Messages will be cloned to each sink.
    #[inline]
    pub fn new(sinks: Vec<Box<dyn LogSink>>) -> Self {
        Self(parking_lot::Mutex::new(sinks))
    }
}

impl LogSink for MultiSink {
    #[inline]
    fn send(&self, msg: LogMsg) {
        for sink in self.0.lock().iter() {
            sink.send(msg.clone());
        }
    }

    #[inline]
    fn send_all(&self, messages: Vec<LogMsg>) {
        for sink in self.0.lock().iter() {
            sink.send_all(messages.clone());
        }
    }

    // Flushes ALL sinks, and returns the most severe error, if any.
    #[inline]
    fn flush_blocking(&self, timeout: Duration) -> Result<(), SinkFlushError> {
        let mut worst_result = Ok(());
        for sink in self.0.lock().iter() {
            if let Err(err) = sink.flush_blocking(timeout)
                && matches!(worst_result, Ok(()) | Err(SinkFlushError::Timeout))
            {
                worst_result = Err(err);
            }
        }
        worst_result
    }

    // NOTE: this is only really used for BufferedSink,
    //       and by the time you `set_sink` you probably don't have
    //       a buffered sink anymore
    #[inline]
    fn drain_backlog(&self) -> Vec<LogMsg> {
        Vec::new()
    }

    fn default_batcher_config(&self) -> ChunkBatcherConfig {
        let ChunkBatcherConfig {
            mut flush_tick,
            mut flush_num_bytes,
            mut flush_num_rows,
            mut chunk_max_rows_if_unsorted,
            mut max_bytes_in_flight,
        } = ChunkBatcherConfig::DEFAULT;

        // Use a mix of the existing sinks thus that we flush *less* often.
        // Prefer less flushing since it leads to better chunks.
        for sink in self.0.lock().iter() {
            let config = sink.default_batcher_config();

            flush_tick = flush_tick.max(config.flush_tick);
            flush_num_bytes = flush_num_bytes.max(config.flush_num_bytes);
            flush_num_rows = flush_num_rows.max(config.flush_num_rows);
            chunk_max_rows_if_unsorted =
                chunk_max_rows_if_unsorted.max(config.chunk_max_rows_if_unsorted);
            max_bytes_in_flight = max_bytes_in_flight.max(config.max_bytes_in_flight);
        }

        ChunkBatcherConfig {
            flush_tick,
            flush_num_bytes,
            flush_num_rows,
            chunk_max_rows_if_unsorted,
            max_bytes_in_flight,
        }
    }
}

mod private {
    pub trait Sealed {}
}

/// Marker trait for [`LogSink`] implementors which may be added
/// to a [`MultiSink`].
pub trait MultiSinkCompatible: private::Sealed {}

/// Conversion trait implemented for tuples of sinks.
pub trait IntoMultiSink {
    /// Convert self into a [`MultiSink`].
    fn into_multi_sink(self) -> MultiSink;
}

macro_rules! impl_multi_sink_tuple {
    ($($T:ident),*) => {
        impl<$($T),*> IntoMultiSink for ($($T,)*)
        where
            $($T: LogSink + MultiSinkCompatible,)*
        {
            #[expect(non_snake_case)] // so that we only need one metavar
            #[inline]
            fn into_multi_sink(self) -> MultiSink {
                let ($($T,)*) = self;
                MultiSink::new(vec![$(Box::new($T)),*])
            }
        }
    };
}

impl_multi_sink_tuple!(A);
impl_multi_sink_tuple!(A, B);
impl_multi_sink_tuple!(A, B, C);
impl_multi_sink_tuple!(A, B, C, D);
impl_multi_sink_tuple!(A, B, C, D, E);
impl_multi_sink_tuple!(A, B, C, D, E, F);

impl IntoMultiSink for Vec<Box<dyn LogSink>> {
    fn into_multi_sink(self) -> MultiSink {
        MultiSink::new(self)
    }
}

impl private::Sealed for crate::sink::FileSink {}

impl MultiSinkCompatible for crate::sink::FileSink {}

impl private::Sealed for crate::sink::GrpcSink {}

impl MultiSinkCompatible for crate::sink::GrpcSink {}

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
    fn flush_blocking(&self, _timeout: Duration) -> Result<(), SinkFlushError> {
        Ok(())
    }
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
    fn flush_blocking(&self, _timeout: Duration) -> Result<(), SinkFlushError> {
        Ok(())
    }

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
        self.rec.flush_blocking().ok();
        self.inner.lock().msgs.len()
    }

    /// Consumes and returns the inner array of [`LogMsg`].
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn take(&self) -> Vec<LogMsg> {
        // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
        // in this flush; it's just a matter of making the table batcher tick early.
        self.rec.flush_blocking().ok();
        std::mem::take(&mut (self.write()))
    }

    /// Convert the stored messages into an in-memory Rerun log file.
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn concat_memory_sinks_as_bytes(sinks: &[&Self]) -> Result<Vec<u8>, EncodeError> {
        let mut encoder = Encoder::local()?;

        for sink in sinks {
            // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
            // in this flush; it's just a matter of making the table batcher tick early.
            sink.rec.flush_blocking().ok();
            let mut inner = sink.inner.lock();
            inner.has_been_used = true;

            for message in &inner.msgs {
                encoder.append(message)?;
            }
        }

        encoder.finish()?;

        encoder.into_inner()
    }

    /// Drain the stored messages and return them as an in-memory RRD.
    ///
    /// This automatically takes care of flushing the underlying [`crate::RecordingStream`].
    #[inline]
    pub fn drain_as_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        // NOTE: It's fine, this is an in-memory sink so by definition there's no I/O involved
        // in this flush; it's just a matter of making the table batcher tick early.
        self.rec.flush_blocking().ok();

        let mut inner = self.inner.lock();
        inner.has_been_used = true;

        Encoder::encode(std::mem::take(&mut inner.msgs).into_iter().map(Ok))
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
    fn flush_blocking(&self, _timeout: Duration) -> Result<(), SinkFlushError> {
        Ok(())
    }
}

// ----------------------------------------------------------------------------

/// Stream log messages to an a remote Rerun server.
pub struct GrpcSink {
    client: MessageProxyClient,
}

/// The connection state of the underlying gRPC connection of a [`GrpcSink`].
pub type GrpcSinkConnectionState = re_grpc_client::write::ClientConnectionState;

/// The reason why a [`GrpcSink`] was disconnected.
pub type GrpcSinkConnectionFailure = re_grpc_client::write::ClientConnectionFailure;

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
    pub fn new(uri: re_uri::ProxyUri) -> Self {
        Self {
            client: MessageProxyClient::new(uri, Options::default()),
        }
    }

    /// The connection state of underlying Grpc connection of this sink.
    ///
    /// # Experimental
    ///
    /// This API is experimental and may change in future releases.
    pub fn status(&self) -> GrpcSinkConnectionState {
        self.client.status()
    }
}

impl Default for GrpcSink {
    fn default() -> Self {
        use std::str::FromStr as _;
        Self::new(
            re_uri::ProxyUri::from_str(crate::DEFAULT_CONNECT_URL).expect("failed to parse uri"),
        )
    }
}

impl LogSink for GrpcSink {
    fn send(&self, mut log_msg: LogMsg) {
        if let Some(metadata_key) = re_sorbet::TimestampLocation::GrpcSink.metadata_key() {
            // Used for latency measurements:
            log_msg.insert_arrow_record_batch_metadata(
                metadata_key.to_owned(),
                re_sorbet::timestamp_metadata::now_timestamp(),
            );
        }

        self.client.send_blocking(log_msg);
    }

    fn flush_blocking(&self, timeout: Duration) -> Result<(), SinkFlushError> {
        self.client
            .flush_blocking(timeout)
            .map_err(|err| match err {
                GrpcFlushError::Timeout { .. } => SinkFlushError::Timeout,
                err => SinkFlushError::failed(err.to_string()),
            })
    }

    fn default_batcher_config(&self) -> ChunkBatcherConfig {
        // The GRPC sink is typically used for live streams.
        ChunkBatcherConfig::LOW_LATENCY
    }
}
