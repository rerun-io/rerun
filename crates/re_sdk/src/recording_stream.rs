use std::sync::Arc;

use crossbeam::channel::{Receiver, Sender};
use re_log_types::{
    ApplicationId, DataRow, DataTable, DataTableBatcher, DataTableBatcherConfig,
    DataTableBatcherError, LogMsg, RecordingId, RecordingInfo, RecordingSource, Time,
};

use crate::sink::{LogSink, MemorySinkStorage};

// ---

/// Errors that can occur when creating/manipulating a [`RecordingStream`].
#[derive(thiserror::Error, Debug)]
pub enum RecordingStreamError {
    /// Error within the underlying table batcher.
    #[error("Failed to spawn the underlying batcher: {0}")]
    DataTableBatcher(#[from] DataTableBatcherError),

    /// Error spawning one of the background threads.
    #[error("Failed to spawn background thread '{name}': {err}")]
    SpawnThread {
        name: &'static str,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type RecordingStreamResult<T> = Result<T, RecordingStreamError>;

// ---

/// Construct a [`RecordingStream`].
///
/// ``` no_run
/// # use re_sdk::RecordingStreamBuilder;
/// let rec_stream = RecordingStreamBuilder::new("my_app").save("my_recording.rrd")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct RecordingStreamBuilder {
    application_id: ApplicationId,
    recording_id: Option<RecordingId>,
    recording_source: Option<RecordingSource>,

    default_enabled: bool,
    enabled: Option<bool>,

    batcher_config: Option<DataTableBatcherConfig>,

    is_official_example: bool,
}

impl RecordingStreamBuilder {
    /// Create a new [`RecordingStreamBuilder`] with the given [`ApplicationId`].
    ///
    /// The [`ApplicationId`] is usually the name of your app.
    ///
    /// ```no_run
    /// # use re_sdk::RecordingStreamBuilder;
    /// let rec_stream = RecordingStreamBuilder::new("my_app").save("my_recording.rrd")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    //
    // NOTE: track_caller so that we can see if we are being called from an official example.
    #[track_caller]
    pub fn new(application_id: impl Into<ApplicationId>) -> Self {
        let application_id = application_id.into();
        let is_official_example = crate::called_from_official_rust_example();

        Self {
            application_id,
            recording_id: None,
            recording_source: None,

            default_enabled: true,
            enabled: None,

            batcher_config: None,
            is_official_example,
        }
    }

    /// Set whether or not Rerun is enabled by default.
    ///
    /// If the `RERUN` environment variable is set, it will override this.
    ///
    /// Set also: [`Self::enabled`].
    pub fn default_enabled(mut self, default_enabled: bool) -> Self {
        self.default_enabled = default_enabled;
        self
    }

    /// Set whether or not Rerun is enabled.
    ///
    /// Setting this will ignore the `RERUN` environment variable.
    ///
    /// Set also: [`Self::default_enabled`].
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Set the [`RecordingId`] for this context.
    ///
    /// If you're logging from multiple processes and want all the messages to end up as the same
    /// recording, you must make sure they all set the same [`RecordingId`] using this function.
    ///
    /// Note that many recordings can share the same [`ApplicationId`], but they all have
    /// unique [`RecordingId`]s.
    ///
    /// The default is to use a random [`RecordingId`].
    pub fn recording_id(mut self, recording_id: RecordingId) -> Self {
        self.recording_id = Some(recording_id);
        self
    }

    /// Specifies the configuration of the internal data batching mechanism.
    ///
    /// See [`DataTableBatcher`] & [`DataTableBatcherConfig`] for more information.
    pub fn batcher_config(mut self, config: DataTableBatcherConfig) -> Self {
        self.batcher_config = Some(config);
        self
    }

    #[doc(hidden)]
    pub fn recording_source(mut self, recording_source: RecordingSource) -> Self {
        self.recording_source = Some(recording_source);
        self
    }

    #[doc(hidden)]
    pub fn is_official_example(mut self, is_official_example: bool) -> Self {
        self.is_official_example = is_official_example;
        self
    }

    /// Creates a new [`RecordingStream`] that starts in a buffering state (RAM).
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec_stream = re_sdk::RecordingStreamBuilder::new("my_app").buffered()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn buffered(self) -> RecordingStreamResult<RecordingStream> {
        let (enabled, recording_info, batcher_config) = self.finalize();
        if enabled {
            RecordingStream::new(
                recording_info,
                batcher_config,
                Box::new(crate::log_sink::BufferedSink::new()),
            )
        } else {
            re_log::debug!("Rerun disabled - call to buffered() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// [`crate::log_sink::MemorySink`].
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let (rec_stream, storage) = re_sdk::RecordingStreamBuilder::new("my_app").memory()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn memory(
        self,
    ) -> RecordingStreamResult<(RecordingStream, crate::log_sink::MemorySinkStorage)> {
        let sink = crate::log_sink::MemorySink::default();
        let storage = sink.buffer();

        let (enabled, recording_info, batcher_config) = self.finalize();
        if enabled {
            RecordingStream::new(recording_info, batcher_config, Box::new(sink))
                .map(|rec_stream| (rec_stream, storage))
        } else {
            re_log::debug!("Rerun disabled - call to memory() ignored");
            Ok((RecordingStream::disabled(), Default::default()))
        }
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// remote Rerun instance.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec_stream = re_sdk::RecordingStreamBuilder::new("my_app")
    ///     .connect(re_sdk::default_server_addr())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn connect(self, addr: std::net::SocketAddr) -> RecordingStreamResult<RecordingStream> {
        let (enabled, recording_info, batcher_config) = self.finalize();
        if enabled {
            RecordingStream::new(
                recording_info,
                batcher_config,
                Box::new(crate::log_sink::TcpSink::new(addr)),
            )
        } else {
            re_log::debug!("Rerun disabled - call to connect() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to an
    /// RRD file on disk.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec_stream = re_sdk::RecordingStreamBuilder::new("my_app").save("my_recording.rrd")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(
        self,
        path: impl Into<std::path::PathBuf>,
    ) -> RecordingStreamResult<RecordingStream> {
        let (enabled, recording_info, batcher_config) = self.finalize();

        if enabled {
            RecordingStream::new(
                recording_info,
                batcher_config,
                Box::new(crate::sink::FileSink::new(path).unwrap()), // TODO
            )
        } else {
            re_log::debug!("Rerun disabled - call to save() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Returns whether or not logging is enabled, a [`RecordingInfo`] and the associated batcher
    /// configuration.
    ///
    /// This can be used to then construct a [`RecordingStream`] manually using
    /// [`RecordingStream::new`].
    pub fn finalize(self) -> (bool, RecordingInfo, DataTableBatcherConfig) {
        let Self {
            application_id,
            recording_id,
            recording_source,
            default_enabled,
            enabled,
            batcher_config,
            is_official_example,
        } = self;

        let enabled = enabled.unwrap_or_else(|| crate::decide_logging_enabled(default_enabled));
        let recording_id = recording_id.unwrap_or_else(RecordingId::random);
        let recording_source = recording_source.unwrap_or_else(|| RecordingSource::RustSdk {
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
        });

        let recording_info = RecordingInfo {
            application_id,
            recording_id,
            is_official_example,
            started: Time::now(),
            recording_source,
        };

        let batcher_config = batcher_config
            .unwrap_or_else(|| DataTableBatcherConfig::from_env().unwrap_or_default());

        (enabled, recording_info, batcher_config)
    }
}

// ----------------------------------------------------------------------------

/// A [`RecordingStream`] handles everything related to logging data into Rerun.
///
/// You can construct a new [`RecordingStream`] using [`RecordingStreamBuilder`] or
/// [`RecordingStream::new`].
///
/// ## Sinks
///
/// Data is logged into Rerun via [`LogSink`]s.
///
/// The underlying [`LogSink`] of a [`RecordingStream`] can be changed at any point during its
/// lifetime by calling [`RecordingStream::set_sink`] or one of the higher level helpers
/// ([`RecordingStream::connect`], [`RecordingStream::memory`],
/// [`RecordingStream::save`], [`RecordingStream::disconnect`]).
///
/// See [`RecordingStream::set_sink`] for more information.
///
/// ## Multithreading and ordering
///
/// [`RecordingStream`] can be cheaply cloned and used freely across any number of threads.
///
/// Internally, all operations are linearized into a pipeline:
/// - All operations sent by a given thread will take effect in the same exact order as that
///   thread originally sent them in, from its point of view.
/// - There isn't any well defined global order across multiple threads.
///
/// This means that e.g. flushing the pipeline ([`Self::flush_blocking`]) guarantees that all
/// previous data sent by the calling thread has been recorded; no more, no less.
///
/// ## Shutdown
///
/// The [`RecordingStream`] can only be shutdown by dropping all instances of it, at which point
/// it will automatically take care of flushing any pending data that might remain in the pipeline.
///
/// Shutting down cannot ever block.
#[derive(Clone)]
pub struct RecordingStream {
    inner: Arc<Option<RecordingStreamInner>>,
}

struct RecordingStreamInner {
    info: RecordingInfo,

    /// The one and only entrypoint into the pipeline: this is _never_ cloned nor publicly exposed,
    /// therefore the `Drop` implementation is guaranteed that no more data can come in while it's
    /// running.
    cmds_tx: Sender<Command>,

    batcher: DataTableBatcher,
    batcher_to_sink_handle: Option<std::thread::JoinHandle<()>>,
    //
    // TODO(emilk): add convenience `TimePoint` here so that users can
    // do things like `session.set_time_sequence("frame", frame_idx);`
}

impl Drop for RecordingStreamInner {
    fn drop(&mut self) {
        // NOTE: The command channel is private, if we're here, nothing is currently capable of
        // sending data down the pipeline.
        self.batcher.flush_blocking();
        self.cmds_tx.send(Command::PopPendingTables).ok();
        self.cmds_tx.send(Command::Shutdown).ok();
        if let Some(handle) = self.batcher_to_sink_handle.take() {
            handle.join().ok();
        }
    }
}

impl RecordingStreamInner {
    fn new(
        info: RecordingInfo,
        batcher_config: DataTableBatcherConfig,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        let batcher = DataTableBatcher::new(batcher_config)?;

        // TODO(cmc): BeginRecordingMsg is a misnomer; it's idempotent.
        {
            re_log::debug!(
                app_id = %info.application_id,
                rec_id = %info.recording_id,
                "setting recording info",
            );
            sink.send(
                re_log_types::BeginRecordingMsg {
                    row_id: re_log_types::RowId::random(),
                    info: info.clone(),
                }
                .into(),
            );
        }

        let (cmds_tx, cmds_rx) = crossbeam::channel::unbounded();

        let batcher_to_sink_handle = {
            const NAME: &str = "RecordingStream::batcher_to_sink";
            std::thread::Builder::new()
                .name(NAME.into())
                .spawn({
                    let info = info.clone();
                    let batcher = batcher.clone();
                    move || forwarding_thread(info, sink, cmds_rx, batcher.tables())
                })
                .map_err(|err| RecordingStreamError::SpawnThread {
                    name: NAME,
                    err: Box::new(err),
                })?
        };

        Ok(RecordingStreamInner {
            info,
            cmds_tx,
            batcher,
            batcher_to_sink_handle: Some(batcher_to_sink_handle),
        })
    }
}

enum Command {
    RecordMsg(LogMsg),
    SwapSink(Box<dyn LogSink>),
    Flush(Sender<()>),
    PopPendingTables,
    Shutdown,
}

impl Command {
    fn flush() -> (Self, Receiver<()>) {
        let (tx, rx) = crossbeam::channel::bounded(0); // oneshot
        (Self::Flush(tx), rx)
    }
}

impl RecordingStream {
    /// Creates a new [`RecordingStream`] with a given [`RecordingInfo`] and [`LogSink`].
    ///
    /// You can create a [`RecordingInfo`] with [`crate::new_recording_info`];
    ///
    /// The [`RecordingInfo`] is immediately sent to the sink in the form of a
    /// [`re_log_types::BeginRecordingMsg`].
    ///
    /// You can find sinks in [`crate::sink`].
    ///
    /// See also: [`RecordingStreamBuilder`].
    #[must_use = "Recording will get closed automatically once all instances of this object have been dropped"]
    pub fn new(
        info: RecordingInfo,
        batcher_config: DataTableBatcherConfig,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        RecordingStreamInner::new(info, batcher_config, sink).map(|inner| Self {
            inner: Arc::new(Some(inner)),
        })
    }

    /// Creates a new no-op [`RecordingStream`] that drops all logging messages, doesn't allocate
    /// any memory and doesn't spawn any threads.
    ///
    /// [`Self::is_enabled`] will return `false`.
    pub fn disabled() -> Self {
        Self {
            inner: Arc::new(None),
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn forwarding_thread(
    info: RecordingInfo,
    mut sink: Box<dyn LogSink>,
    cmds_rx: Receiver<Command>,
    tables: Receiver<DataTable>,
) {
    fn handle_cmd(info: &RecordingInfo, cmd: Command, sink: &mut Box<dyn LogSink>) -> bool {
        match cmd {
            Command::RecordMsg(msg) => {
                sink.send(msg);
            }
            Command::SwapSink(new_sink) => {
                let backlog = {
                    // Capture the backlog if it exists.
                    let backlog = sink.drain_backlog();

                    // Flush the underlying sink if possible.
                    sink.drop_if_disconnected();
                    sink.flush_blocking();

                    backlog
                };

                // Send the recording info to the new sink. This is idempotent.
                {
                    re_log::debug!(
                        app_id = %info.application_id,
                        rec_id = %info.recording_id,
                        "setting recording info",
                    );
                    new_sink.send(
                        re_log_types::BeginRecordingMsg {
                            row_id: re_log_types::RowId::random(),
                            info: info.clone(),
                        }
                        .into(),
                    );
                    new_sink.send_all(backlog);
                }

                *sink = new_sink;
            }
            Command::Flush(oneshot) => {
                // Flush the underlying sink if possible.
                sink.drop_if_disconnected();
                sink.flush_blocking();
                drop(oneshot); // signals the oneshot
            }
            Command::PopPendingTables => {
                // No need to do anything, just skip the current iteration.
            }
            Command::Shutdown => return false,
        }

        true
    }

    use crossbeam::select;
    loop {
        // NOTE: Always pop tables first, this is what makes `Command::PopPendingTables` possible,
        // which in turns makes `RecordingStream::flush_blocking` well defined.
        while let Ok(table) = tables.try_recv() {
            let table = match table.to_arrow_msg() {
                Ok(table) => table,
                Err(err) => {
                    re_log::error!(%err,
                        "couldn't serialize table; data dropped (this is a bug in Rerun!)");
                    continue;
                }
            };
            sink.send(LogMsg::ArrowMsg(info.recording_id, table));
        }

        select! {
            recv(tables) -> res => {
                let Ok(table) = res else {
                    // The batcher is gone, which can only happen if the `RecordingStream` itself
                    // has been dropped.
                    break;
                };
                let table = match table.to_arrow_msg() {
                    Ok(table) => table,
                    Err(err) => {
                        re_log::error!(%err,
                            "couldn't serialize table; data dropped (this is a bug in Rerun!)");
                        continue;
                    }
                };
                sink.send(LogMsg::ArrowMsg(info.recording_id, table));
            }
            recv(cmds_rx) -> res => {
                let Ok(cmd) = res else {
                    // All command senders are gone, which can only happen if the
                    // `RecordingStream` itself has been dropped.
                    break;
                };
                if !handle_cmd(&info, cmd, &mut sink) {
                    break; // shutdown
                }
            }
        }

        // NOTE: The receiving end of the command stream is owned solely by this thread.
        // Past this point, all command writes will return `ErrDisconnected`.
    }
}

impl RecordingStream {
    /// Check if logging is enabled on this `RecordingStream`.
    ///
    /// If not, all recording calls will be ignored.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// The [`RecordingInfo`] associated with this `RecordingStream`.
    #[inline]
    pub fn recording_info(&self) -> Option<&RecordingInfo> {
        (*self.inner).as_ref().map(|inner| &inner.info)
    }
}

impl RecordingStream {
    /// Records an arbitrary [`LogMsg`].
    #[inline]
    pub fn record_msg(&self, msg: LogMsg) {
        let Some(this) = &*self.inner else {
            re_log::debug!("Recording disabled - call to record_msg() ignored");
            return;
        };

        // NOTE: Internal channels can never be closed outside of the `Drop` impl, this send cannot
        // fail.

        this.cmds_tx.send(Command::RecordMsg(msg)).ok();
    }

    /// Records a [`re_log_types::PathOp`].
    ///
    /// This is a convenience wrapper for [`Self::record_msg`].
    #[inline]
    pub fn record_path_op(
        &self,
        timepoint: re_log_types::TimePoint,
        path_op: re_log_types::PathOp,
    ) {
        let Some(this) = &*self.inner else {
            re_log::debug!("Recording disabled - call to record_path_op() ignored");
            return;
        };

        self.record_msg(LogMsg::EntityPathOpMsg(
            this.info.recording_id,
            re_log_types::EntityPathOpMsg {
                row_id: re_log_types::RowId::random(),
                time_point: timepoint,
                path_op,
            },
        ));
    }

    /// Records a single [`DataRow`].
    ///
    /// Internally, incoming [`DataRow`]s are automatically coalesced into larger [`DataTable`]s to
    /// optimize for transport.
    #[inline]
    pub fn record_row(&self, row: DataRow) {
        let Some(this) = &*self.inner else {
            re_log::debug!("Recording disabled - call to record_row() ignored");
            return;
        };

        this.batcher.push_row(row);
    }

    /// Swaps the underlying sink for a new one.
    ///
    /// This guarantees that:
    /// 1. all pending rows and tables are batched, collected and sent down the current sink,
    /// 2. the current sink is flushed if it has pending data in its buffers,
    /// 3. the current sink's backlog, if there's any, is forwarded to the new sink.
    ///
    /// When this function returns, the calling thread is guaranteed that all future record calls
    /// will end up in the new sink.
    ///
    /// ## Data loss
    ///
    /// If the current sink is in a broken state (e.g. a TCP sink with a broken connection that
    /// cannot be repaired), all pending data in its buffers will be dropped.
    pub fn set_sink(&self, sink: Box<dyn LogSink>) {
        let Some(this) = &*self.inner else {
            re_log::debug!("Recording disabled - call to set_sink() ignored");
            return;
        };

        // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
        // are safe.

        // 1. Flush the batcher down the table channel
        this.batcher.flush_blocking();

        // 2. Receive pending tables from the batcher's channel
        this.cmds_tx.send(Command::PopPendingTables).ok();

        // 3. Swap the sink, which will internally make sure to re-ingest the backlog if needed
        this.cmds_tx.send(Command::SwapSink(sink)).ok();

        // 4. Before we give control back to the caller, we need to make sure that the swap has
        //    taken place: we don't want the user to send data to the old sink!
        let (cmd, oneshot) = Command::flush();
        this.cmds_tx.send(cmd).ok();
        oneshot.recv().ok();
    }

    /// Initiates a flush of the pipeline and returns immediately.
    ///
    /// This does **not** wait for the flush to propagate (see [`Self::flush_blocking`]).
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    pub fn flush_async(&self) {
        let Some(this) = &*self.inner else {
            re_log::debug!("Recording disabled - call to flush_async() ignored");
            return;
        };

        // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
        // are safe.

        // 1. Flush the batcher down the table channel
        this.batcher.flush_blocking();

        // 2. Receive pending tables from the batcher's channel
        this.cmds_tx.send(Command::PopPendingTables).ok();
    }

    /// Initiates a flush the batching pipeline and waits for it to propagate.
    ///
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    pub fn flush_blocking(&self) {
        let Some(this) = &*self.inner else {
            re_log::debug!("Recording disabled - call to flush_blocking() ignored");
            return;
        };

        // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
        // are safe.

        self.flush_async();

        // 3. Wait for all tables to have been forwarded
        let (cmd, oneshot) = Command::flush();
        this.cmds_tx.send(cmd).ok();
        oneshot.recv().ok();
    }
}

impl RecordingStream {
    /// Swaps the underlying sink for a [`crate::log_sink::TcpSink`] sink pre-configured to use
    /// the specified address.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn connect(&self, addr: std::net::SocketAddr) {
        self.set_sink(Box::new(crate::log_sink::TcpSink::new(addr)));
    }

    /// Swaps the underlying sink for a [`crate::sink::MemorySink`] sink and returns the associated
    /// [`MemorySinkStorage`].
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn memory(&self) -> MemorySinkStorage {
        let sink = crate::sink::MemorySink::default();
        let buffer = sink.buffer();
        self.set_sink(Box::new(sink));
        buffer
    }

    /// Swaps the underlying sink for a [`crate::sink::FileSink`] at the specified `path`.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn save(
        &self,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<(), crate::sink::FileSinkError> {
        let sink = crate::sink::FileSink::new(path)?;
        self.set_sink(Box::new(sink));
        Ok(())
    }

    /// Swaps the underlying sink for a [`crate::sink::BufferedSink`].
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn disconnect(&self) {
        self.set_sink(Box::new(crate::sink::BufferedSink::new()));
    }
}

// TODO: some regression tests and we're all good
// TODO: an example would probably be nice too

#[cfg(test)]
mod tests {
    use re_log_types::RowId;

    use super::*;

    #[test]
    fn impl_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RecordingStream>();
    }

    #[test]
    fn never_flush() {
        let rec_stream = RecordingStreamBuilder::new("never_flush")
            .enabled(true)
            .batcher_config(DataTableBatcherConfig::NEVER)
            .buffered()
            .unwrap();

        let rec_info = rec_stream.recording_info().cloned().unwrap();

        let mut table = DataTable::example(false);
        table.compute_all_size_bytes();
        for row in table.to_rows() {
            rec_stream.record_row(row);
        }

        let storage = rec_stream.memory();
        let mut msgs = {
            let mut msgs = storage.take();
            msgs.reverse();
            msgs
        };

        // First message should be a set_recording_info resulting from the original sink swap to
        // buffered mode.
        match msgs.pop().unwrap() {
            LogMsg::BeginRecordingMsg(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(rec_info, msg.info);
            }
            _ => panic!("expected BeginRecordingMsg"),
        }

        // Second message should be a set_recording_info resulting from the later sink swap from
        // buffered mode into in-memory mode.
        // This arrives _before_ the data itself since we're using manual flushing.
        match msgs.pop().unwrap() {
            LogMsg::BeginRecordingMsg(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(rec_info, msg.info);
            }
            _ => panic!("expected BeginRecordingMsg"),
        }

        // Third message is the batched table itself, which was sent as a result of the implicit
        // flush when swapping the underlying sink from buffered to in-memory.
        match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(rec_info.recording_id, rid);

                let mut got = DataTable::from_arrow_msg(&msg).unwrap();
                // TODO(1760): we shouldn't have to (re)do this!
                got.compute_all_size_bytes();
                // NOTE: Override the resulting table's ID so they can be compared.
                got.table_id = table.table_id;

                similar_asserts::assert_eq!(table, got);
            }
            _ => panic!("expected BeginRecordingMsg"),
        }

        // That's all.
        assert!(msgs.pop().is_none());
    }

    #[test]
    fn always_flush() {
        let rec_stream = RecordingStreamBuilder::new("always_flush")
            .enabled(true)
            .batcher_config(DataTableBatcherConfig::ALWAYS)
            .buffered()
            .unwrap();

        let rec_info = rec_stream.recording_info().cloned().unwrap();

        let mut table = DataTable::example(false);
        table.compute_all_size_bytes();
        for row in table.to_rows() {
            rec_stream.record_row(row);
        }

        let storage = rec_stream.memory();
        let mut msgs = {
            let mut msgs = storage.take();
            msgs.reverse();
            msgs
        };

        // First message should be a set_recording_info resulting from the original sink swap to
        // buffered mode.
        match msgs.pop().unwrap() {
            LogMsg::BeginRecordingMsg(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(rec_info, msg.info);
            }
            _ => panic!("expected BeginRecordingMsg"),
        }

        // Second message should be a set_recording_info resulting from the later sink swap from
        // buffered mode into in-memory mode.
        // This arrives _before_ the data itself since we're using manual flushing.
        match msgs.pop().unwrap() {
            LogMsg::BeginRecordingMsg(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(rec_info, msg.info);
            }
            _ => panic!("expected BeginRecordingMsg"),
        }

        let mut rows = {
            let mut rows: Vec<_> = table.to_rows().collect();
            rows.reverse();
            rows
        };

        let mut assert_next_row = || {
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(rec_info.recording_id, rid);

                    let mut got = DataTable::from_arrow_msg(&msg).unwrap();
                    // TODO(1760): we shouldn't have to (re)do this!
                    got.compute_all_size_bytes();
                    // NOTE: Override the resulting table's ID so they can be compared.
                    got.table_id = table.table_id;

                    let expected = DataTable::from_rows(got.table_id, [rows.pop().unwrap()]);

                    similar_asserts::assert_eq!(expected, got);
                }
                _ => panic!("expected BeginRecordingMsg"),
            }
        };

        // 3rd, 4th and 5th messages are all the single-row batched tables themselves, which were
        // sent as a result of the implicit flush when swapping the underlying sink from buffered
        // to in-memory.
        assert_next_row();
        assert_next_row();
        assert_next_row();

        // That's all.
        assert!(msgs.pop().is_none());
    }
}
