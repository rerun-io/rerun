use std::fmt;
use std::sync::{atomic::AtomicI64, Arc};

use ahash::HashMap;
use crossbeam::channel::{Receiver, Sender};

use re_log_types::{
    ApplicationId, ArrowChunkReleaseCallback, DataCell, DataCellError, DataRow, DataTable,
    DataTableBatcher, DataTableBatcherConfig, DataTableBatcherError, EntityPath, LogMsg, RowId,
    StoreId, StoreInfo, StoreKind, StoreSource, Time, TimeInt, TimePoint, TimeType, Timeline,
    TimelineName,
};
use re_types_core::{components::InstanceKey, AsComponents, ComponentBatch, SerializationError};

#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "web_viewer")]
use re_ws_comms::RerunServerPort;

use crate::sink::{LogSink, MemorySinkStorage};

// ---

/// Private environment variable meant for tests.
///
/// When set, all recording streams will write to disk at the path indicated by the env-var rather
/// than doing what they were asked to do - `connect()`, `buffered()`, even `save()` will re-use the same sink.
const ENV_FORCE_SAVE: &str = "_RERUN_TEST_FORCE_SAVE";

/// Returns path for force sink if private environment variable `_RERUN_TEST_FORCE_SAVE` is set
///
/// Newly created [`RecordingStream`]s should use a [`crate::sink::FileSink`] pointing to this path.
/// Furthermore, [`RecordingStream::set_sink`] calls after this should not swap out to a new sink but re-use the existing one.
/// Note that creating a new [`crate::sink::FileSink`] to the same file path (even temporarily) can cause
/// a race between file creation (and thus clearing) and pending file writes.
fn forced_sink_path() -> Option<String> {
    std::env::var(ENV_FORCE_SAVE).ok()
}

/// Errors that can occur when creating/manipulating a [`RecordingStream`].
#[derive(thiserror::Error, Debug)]
pub enum RecordingStreamError {
    /// Error within the underlying file sink.
    #[error("Failed to create the underlying file sink: {0}")]
    FileSink(#[from] re_log_encoding::FileSinkError),

    /// Error within the underlying table batcher.
    #[error("Failed to spawn the underlying batcher: {0}")]
    DataTableBatcher(#[from] DataTableBatcherError),

    /// Error within the underlying data cell.
    #[error("Failed to instantiate data cell: {0}")]
    DataCell(#[from] DataCellError),

    /// Error within the underlying serializer.
    #[error("Failed to serialize component data: {0}")]
    Serialization(#[from] SerializationError),

    /// Error spawning one of the background threads.
    #[error("Failed to spawn background thread '{name}': {err}")]
    SpawnThread {
        /// Name of the thread
        name: &'static str,

        /// Inner error explaining why the thread failed to spawn.
        err: std::io::Error,
    },

    /// Error spawning a Rerun Viewer process.
    #[error(transparent)] // makes bubbling all the way up to main look nice
    SpawnViewer(#[from] crate::SpawnError),

    /// Failure to host a web viewer and/or Rerun server.
    #[cfg(feature = "web_viewer")]
    #[error(transparent)]
    WebSink(#[from] crate::web_viewer::WebViewerSinkError),

    /// An error that can occur because a row in the store has inconsistent columns.
    #[error(transparent)]
    DataReadError(#[from] re_log_types::DataReadError),
}

/// Results that can occur when creating/manipulating a [`RecordingStream`].
pub type RecordingStreamResult<T> = Result<T, RecordingStreamError>;

// ---

/// Construct a [`RecordingStream`].
///
/// ``` no_run
/// # use re_sdk::RecordingStreamBuilder;
/// let rec = RecordingStreamBuilder::new("rerun_example_app").save("my_recording.rrd")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct RecordingStreamBuilder {
    application_id: ApplicationId,
    store_kind: StoreKind,
    store_id: Option<StoreId>,
    store_source: Option<StoreSource>,

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
    /// let rec = RecordingStreamBuilder::new("rerun_example_app").save("my_recording.rrd")?;
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
            store_kind: StoreKind::Recording,
            store_id: None,
            store_source: None,

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
    #[inline]
    pub fn default_enabled(mut self, default_enabled: bool) -> Self {
        self.default_enabled = default_enabled;
        self
    }

    /// Set whether or not Rerun is enabled.
    ///
    /// Setting this will ignore the `RERUN` environment variable.
    ///
    /// Set also: [`Self::default_enabled`].
    #[inline]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Set the `RecordingId` for this context.
    ///
    /// If you're logging from multiple processes and want all the messages to end up in the same
    /// recording, you must make sure that they all set the same `RecordingId` using this function.
    ///
    /// Note that many stores can share the same [`ApplicationId`], but they all have
    /// unique `RecordingId`s.
    ///
    /// The default is to use a random `RecordingId`.
    #[inline]
    pub fn recording_id(mut self, recording_id: impl Into<String>) -> Self {
        self.store_id = Some(StoreId::from_string(
            StoreKind::Recording,
            recording_id.into(),
        ));
        self
    }

    /// Set the [`StoreId`] for this context.
    ///
    /// If you're logging from multiple processes and want all the messages to end up as the same
    /// store, you must make sure they all set the same [`StoreId`] using this function.
    ///
    /// Note that many stores can share the same [`ApplicationId`], but they all have
    /// unique [`StoreId`]s.
    ///
    /// The default is to use a random [`StoreId`].
    #[inline]
    pub fn store_id(mut self, store_id: StoreId) -> Self {
        self.store_id = Some(store_id);
        self
    }

    /// Specifies the configuration of the internal data batching mechanism.
    ///
    /// See [`DataTableBatcher`] & [`DataTableBatcherConfig`] for more information.
    #[inline]
    pub fn batcher_config(mut self, config: DataTableBatcherConfig) -> Self {
        self.batcher_config = Some(config);
        self
    }

    #[doc(hidden)]
    #[inline]
    pub fn store_source(mut self, store_source: StoreSource) -> Self {
        self.store_source = Some(store_source);
        self
    }

    #[allow(clippy::wrong_self_convention)]
    #[doc(hidden)]
    #[inline]
    pub fn is_official_example(mut self, is_official_example: bool) -> Self {
        self.is_official_example = is_official_example;
        self
    }

    #[doc(hidden)]
    #[inline]
    pub fn blueprint(mut self) -> Self {
        self.store_kind = StoreKind::Blueprint;
        self
    }

    /// Creates a new [`RecordingStream`] that starts in a buffering state (RAM).
    ///
    /// ## Example
    ///
    /// ```
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app").buffered()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn buffered(self) -> RecordingStreamResult<RecordingStream> {
        let (enabled, store_info, batcher_config) = self.into_args();
        if enabled {
            RecordingStream::new(
                store_info,
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
    /// ```
    /// # fn log_data(_: &re_sdk::RecordingStream) { }
    ///
    /// let (rec, storage) = re_sdk::RecordingStreamBuilder::new("rerun_example_app").memory()?;
    ///
    /// log_data(&rec);
    ///
    /// let data = storage.take();
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn memory(
        self,
    ) -> RecordingStreamResult<(RecordingStream, crate::log_sink::MemorySinkStorage)> {
        let sink = crate::log_sink::MemorySink::default();
        let mut storage = sink.buffer();

        let (enabled, store_info, batcher_config) = self.into_args();
        if enabled {
            RecordingStream::new(store_info, batcher_config, Box::new(sink)).map(|rec| {
                storage.rec = Some(rec.clone());
                (rec, storage)
            })
        } else {
            re_log::debug!("Rerun disabled - call to memory() ignored");
            Ok((RecordingStream::disabled(), Default::default()))
        }
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// remote Rerun instance.
    ///
    /// See also [`Self::connect_opts`] if you wish to configure the TCP connection.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app").connect()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn connect(self) -> RecordingStreamResult<RecordingStream> {
        self.connect_opts(crate::default_server_addr(), crate::default_flush_timeout())
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// remote Rerun instance.
    ///
    /// `flush_timeout` is the minimum time the [`TcpSink`][`crate::log_sink::TcpSink`] will
    /// wait during a flush before potentially dropping data.  Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app")
    ///     .connect_opts(re_sdk::default_server_addr(), re_sdk::default_flush_timeout())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn connect_opts(
        self,
        addr: std::net::SocketAddr,
        flush_timeout: Option<std::time::Duration>,
    ) -> RecordingStreamResult<RecordingStream> {
        let (enabled, store_info, batcher_config) = self.into_args();
        if enabled {
            RecordingStream::new(
                store_info,
                batcher_config,
                Box::new(crate::log_sink::TcpSink::new(addr, flush_timeout)),
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
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app").save("my_recording.rrd")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(
        self,
        path: impl Into<std::path::PathBuf>,
    ) -> RecordingStreamResult<RecordingStream> {
        let (enabled, store_info, batcher_config) = self.into_args();

        if enabled {
            RecordingStream::new(
                store_info,
                batcher_config,
                Box::new(crate::sink::FileSink::new(path)?),
            )
        } else {
            re_log::debug!("Rerun disabled - call to save() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to stdout.
    ///
    /// If there isn't any listener at the other end of the pipe, the [`RecordingStream`] will
    /// default back to `buffered` mode, in order not to break the user's terminal.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app").stdout()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn stdout(self) -> RecordingStreamResult<RecordingStream> {
        let is_stdout_listening = !atty::is(atty::Stream::Stdout);
        if !is_stdout_listening {
            return self.buffered();
        }

        let (enabled, store_info, batcher_config) = self.into_args();

        if enabled {
            RecordingStream::new(
                store_info,
                batcher_config,
                Box::new(crate::sink::FileSink::stdout()?),
            )
        } else {
            re_log::debug!("Rerun disabled - call to stdout() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Spawns a new Rerun Viewer process from an executable available in PATH, then creates a new
    /// [`RecordingStream`] that is pre-configured to stream the data through to that viewer over TCP.
    ///
    /// If a Rerun Viewer is already listening on this TCP port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// See also [`Self::spawn_opts`] if you wish to configure the behavior of thew Rerun process
    /// as well as the underlying TCP connection.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app").spawn()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn spawn(self) -> RecordingStreamResult<RecordingStream> {
        self.spawn_opts(&Default::default(), crate::default_flush_timeout())
    }

    /// Spawns a new Rerun Viewer process from an executable available in PATH, then creates a new
    /// [`RecordingStream`] that is pre-configured to stream the data through to that viewer over TCP.
    ///
    /// If a Rerun Viewer is already listening on this TCP port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// The behavior of the spawned Viewer can be configured via `opts`.
    /// If you're fine with the default behavior, refer to the simpler [`Self::spawn`].
    ///
    /// `flush_timeout` is the minimum time the [`TcpSink`][`crate::log_sink::TcpSink`] will
    /// wait during a flush before potentially dropping data.  Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app")
    ///     .spawn_opts(&re_sdk::SpawnOptions::default(), re_sdk::default_flush_timeout())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn spawn_opts(
        self,
        opts: &crate::SpawnOptions,
        flush_timeout: Option<std::time::Duration>,
    ) -> RecordingStreamResult<RecordingStream> {
        let connect_addr = opts.connect_addr();

        // NOTE: If `_RERUN_TEST_FORCE_SAVE` is set, all recording streams will write to disk no matter
        // what, thus spawning a viewer is pointless (and probably not intended).
        if forced_sink_path().is_some() {
            return self.connect_opts(connect_addr, flush_timeout);
        }

        spawn(opts)?;

        self.connect_opts(connect_addr, flush_timeout)
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// web-based Rerun viewer via WebSockets.
    ///
    /// This method needs to be called in a context where a Tokio runtime is already running (see
    /// example below).
    ///
    /// If the `open_browser` argument is `true`, your default browser will be opened with a
    /// connected web-viewer.
    ///
    /// If not, you can connect to this server using the `rerun` binary (`cargo install rerun-cli`).
    ///
    /// ## Details
    /// This method will spawn two servers: one HTTPS server serving the Rerun Web Viewer `.html` and `.wasm` files,
    /// and then one WebSocket server that streams the log data to the web viewer (or to a native viewer, or to multiple viewers).
    ///
    /// The WebSocket server will buffer all log data in memory so that late connecting viewers will get all the data.
    /// You can limit the amount of data buffered by the WebSocket server with the `server_memory_limit` argument.
    /// Once reached, the earliest logged data will be dropped.
    /// Note that this means that timeless data may be dropped if logged early.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Ensure we have a running tokio runtime.
    /// let mut tokio_runtime = None;
    /// let tokio_runtime_handle = if let Ok(handle) = tokio::runtime::Handle::try_current() {
    ///     handle
    /// } else {
    ///     let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    ///     tokio_runtime.get_or_insert(rt).handle().clone()
    /// };
    /// let _tokio_runtime_guard = tokio_runtime_handle.enter();
    ///
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app")
    ///     .serve("0.0.0.0",
    ///            Default::default(),
    ///            Default::default(),
    ///            re_sdk::MemoryLimit::from_fraction_of_total(0.25),
    ///            true)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "web_viewer")]
    pub fn serve(
        self,
        bind_ip: &str,
        web_port: WebViewerServerPort,
        ws_port: RerunServerPort,
        server_memory_limit: re_memory::MemoryLimit,
        open_browser: bool,
    ) -> RecordingStreamResult<RecordingStream> {
        let (enabled, store_info, batcher_config) = self.into_args();
        if enabled {
            let sink = crate::web_viewer::new_sink(
                open_browser,
                bind_ip,
                web_port,
                ws_port,
                server_memory_limit,
            )?;
            RecordingStream::new(store_info, batcher_config, sink)
        } else {
            re_log::debug!("Rerun disabled - call to serve() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Returns whether or not logging is enabled, a [`StoreInfo`] and the associated batcher
    /// configuration.
    ///
    /// This can be used to then construct a [`RecordingStream`] manually using
    /// [`RecordingStream::new`].
    pub fn into_args(self) -> (bool, StoreInfo, DataTableBatcherConfig) {
        let Self {
            application_id,
            store_kind,
            store_id,
            store_source,
            default_enabled,
            enabled,
            batcher_config,
            is_official_example,
        } = self;

        let enabled = enabled.unwrap_or_else(|| crate::decide_logging_enabled(default_enabled));
        let store_id = store_id.unwrap_or(StoreId::random(store_kind));
        let store_source = store_source.unwrap_or_else(|| StoreSource::RustSdk {
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
        });

        let store_info = StoreInfo {
            application_id,
            store_id,
            is_official_example,
            started: Time::now(),
            store_source,
            store_kind,
        };

        let batcher_config = batcher_config
            .unwrap_or_else(|| DataTableBatcherConfig::from_env().unwrap_or_default());

        (enabled, store_info, batcher_config)
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
/// (e.g. it does not mean that all file caches are flushed)
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
    info: StoreInfo,
    tick: AtomicI64,

    /// The one and only entrypoint into the pipeline: this is _never_ cloned nor publicly exposed,
    /// therefore the `Drop` implementation is guaranteed that no more data can come in while it's
    /// running.
    cmds_tx: Sender<Command>,

    batcher: DataTableBatcher,
    batcher_to_sink_handle: Option<std::thread::JoinHandle<()>>,

    pid_at_creation: u32,
}

impl Drop for RecordingStreamInner {
    fn drop(&mut self) {
        if self.is_forked_child() {
            re_log::error_once!("Fork detected while dropping RecordingStreamInner. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK.");
            return;
        }

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
        info: StoreInfo,
        batcher_config: DataTableBatcherConfig,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        let on_release = batcher_config.hooks.on_release.clone();
        let batcher = DataTableBatcher::new(batcher_config)?;

        {
            re_log::debug!(
                app_id = %info.application_id,
                rec_id = %info.store_id,
                "setting recording info",
            );
            sink.send(
                re_log_types::SetStoreInfo {
                    row_id: re_log_types::RowId::new(),
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
                    move || forwarding_thread(info, sink, cmds_rx, batcher.tables(), on_release)
                })
                .map_err(|err| RecordingStreamError::SpawnThread { name: NAME, err })?
        };

        Ok(RecordingStreamInner {
            info,
            tick: AtomicI64::new(0),
            cmds_tx,
            batcher,
            batcher_to_sink_handle: Some(batcher_to_sink_handle),
            pid_at_creation: std::process::id(),
        })
    }

    #[inline]
    pub fn is_forked_child(&self) -> bool {
        self.pid_at_creation != std::process::id()
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
    /// Creates a new [`RecordingStream`] with a given [`StoreInfo`] and [`LogSink`].
    ///
    /// You can create a [`StoreInfo`] with [`crate::new_store_info`];
    ///
    /// The [`StoreInfo`] is immediately sent to the sink in the form of a
    /// [`re_log_types::SetStoreInfo`].
    ///
    /// You can find sinks in [`crate::sink`].
    ///
    /// See also: [`RecordingStreamBuilder`].
    #[must_use = "Recording will get closed automatically once all instances of this object have been dropped"]
    pub fn new(
        info: StoreInfo,
        batcher_config: DataTableBatcherConfig,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        let sink = forced_sink_path().map_or(sink, |path| {
            re_log::info!("Forcing FileSink because of env-var {ENV_FORCE_SAVE}={path:?}");
            // `unwrap` is ok since this force sinks are only used in tests.
            Box::new(crate::sink::FileSink::new(path).unwrap()) as Box<dyn LogSink>
        });
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

impl RecordingStream {
    /// Log data to Rerun.
    ///
    /// This is the main entry point for logging data to rerun. It can be used to log anything
    /// that implements the [`AsComponents`], such as any [archetype](https://docs.rs/rerun/latest/rerun/archetypes/index.html).
    ///
    /// The data will be timestamped automatically based on the [`RecordingStream`]'s internal clock.
    /// See [`RecordingStream::set_time_sequence`] etc for more information.
    ///
    /// The entity path can either be a string
    /// (with special characters escaped, split on unescaped slashes)
    /// or an [`EntityPath`] constructed with [`crate::entity_path`].
    /// See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
    ///
    /// See also: [`Self::log_timeless`] for logging timeless data.
    ///
    /// Internally, the stream will automatically micro-batch multiple log calls to optimize
    /// transport.
    /// See [SDK Micro Batching] for more information.
    ///
    /// # Example:
    /// ```ignore
    /// # use rerun;
    /// # let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_points3d_simple").memory()?;
    /// rec.log(
    ///     "my/points",
    ///     &rerun::Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]),
    /// )?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk-micro-batching
    /// [component bundle]: [`AsComponents`]
    #[inline]
    pub fn log(
        &self,
        ent_path: impl Into<EntityPath>,
        arch: &impl AsComponents,
    ) -> RecordingStreamResult<()> {
        self.log_with_timeless(ent_path, false, arch)
    }

    /// Log data to Rerun.
    ///
    /// It can be used to log anything
    /// that implements the [`AsComponents`], such as any [archetype](https://docs.rs/rerun/latest/rerun/archetypes/index.html).
    ///
    /// Timeless data is present on all timelines and behaves as if it was recorded infinitely far
    /// into the past.
    /// All timestamp data associated with this message will be dropped right before sending it to Rerun.
    ///
    /// This is most often used for [`rerun::ViewCoordinates`](https://docs.rs/rerun/latest/rerun/archetypes/struct.ViewCoordinates.html) and
    /// [`rerun::AnnotationContext`](https://docs.rs/rerun/latest/rerun/archetypes/struct.AnnotationContext.html).
    ///
    /// Internally, the stream will automatically micro-batch multiple log calls to optimize
    /// transport.
    /// See [SDK Micro Batching] for more information.
    ///
    /// See also [`Self::log`].
    ///
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk-micro-batching
    /// [component bundle]: [`AsComponents`]
    #[inline]
    pub fn log_timeless(
        &self,
        ent_path: impl Into<EntityPath>,
        arch: &impl AsComponents,
    ) -> RecordingStreamResult<()> {
        self.log_with_timeless(ent_path, true, arch)
    }

    /// Logs the contents of a [component bundle] into Rerun.
    ///
    /// If `timeless` is set to `true`, all timestamp data associated with this message will be
    /// dropped right before sending it to Rerun.
    /// Timeless data is present on all timelines and behaves as if it was recorded infinitely far
    /// into the past.
    ///
    /// Otherwise, the data will be timestamped automatically based on the [`RecordingStream`]'s
    /// internal clock.
    /// See `RecordingStream::set_time_*` family of methods for more information.
    ///
    /// The entity path can either be a string
    /// (with special characters escaped, split on unescaped slashes)
    /// or an [`EntityPath`] constructed with [`crate::entity_path`].
    /// See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
    ///
    /// Internally, the stream will automatically micro-batch multiple log calls to optimize
    /// transport.
    /// See [SDK Micro Batching] for more information.
    ///
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk-micro-batching
    /// [component bundle]: [`AsComponents`]
    #[inline]
    pub fn log_with_timeless(
        &self,
        ent_path: impl Into<EntityPath>,
        timeless: bool,
        arch: &impl AsComponents,
    ) -> RecordingStreamResult<()> {
        let row_id = RowId::new(); // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.
        self.log_component_batches_impl(
            row_id,
            ent_path,
            timeless,
            arch.as_component_batches()
                .iter()
                .map(|any_comp_batch| any_comp_batch.as_ref()),
        )
    }

    /// Logs a set of [`ComponentBatch`]es into Rerun.
    ///
    /// If `timeless` is set to `false`, all timestamp data associated with this message will be
    /// dropped right before sending it to Rerun.
    /// Timeless data is present on all timelines and behaves as if it was recorded infinitely far
    /// into the past.
    ///
    /// Otherwise, the data will be timestamped automatically based on the [`RecordingStream`]'s
    /// internal clock.
    /// See `RecordingStream::set_time_*` family of methods for more information.
    ///
    /// The number of instances will be determined by the longest batch in the bundle.
    /// All of the batches should have the same number of instances, or length 1 if the component is
    /// a splat, or 0 if the component is being cleared.
    ///
    /// The entity path can either be a string
    /// (with special characters escaped, split on unescaped slashes)
    /// or an [`EntityPath`] constructed with [`crate::entity_path`].
    /// See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
    ///
    /// Internally, the stream will automatically micro-batch multiple log calls to optimize
    /// transport.
    /// See [SDK Micro Batching] for more information.
    ///
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk-micro-batching
    pub fn log_component_batches<'a>(
        &self,
        ent_path: impl Into<EntityPath>,
        timeless: bool,
        comp_batches: impl IntoIterator<Item = &'a dyn ComponentBatch>,
    ) -> RecordingStreamResult<()> {
        let row_id = RowId::new(); // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.
        self.log_component_batches_impl(row_id, ent_path, timeless, comp_batches)
    }

    fn log_component_batches_impl<'a>(
        &self,
        row_id: RowId,
        ent_path: impl Into<EntityPath>,
        timeless: bool,
        comp_batches: impl IntoIterator<Item = &'a dyn ComponentBatch>,
    ) -> RecordingStreamResult<()> {
        if !self.is_enabled() {
            return Ok(()); // silently drop the message
        }

        let ent_path = ent_path.into();

        let mut num_instances = 0;
        let comp_batches: Result<Vec<_>, _> = comp_batches
            .into_iter()
            .map(|comp_batch| {
                num_instances = usize::max(num_instances, comp_batch.num_instances());
                comp_batch
                    .to_arrow()
                    .map(|array| (comp_batch.arrow_field(), array))
            })
            .collect();
        let comp_batches = comp_batches?;

        let cells: Result<Vec<_>, _> = comp_batches
            .into_iter()
            .map(|(field, array)| {
                // NOTE: Unreachable, a top-level Field will always be a component, and thus an
                // extension.
                use re_log_types::external::arrow2::datatypes::DataType;
                let DataType::Extension(fqname, _, _) = field.data_type else {
                    return Err(SerializationError::missing_extension_metadata(field.name).into());
                };
                DataCell::try_from_arrow(fqname.into(), array)
            })
            .collect();
        let cells = cells?;

        let mut instanced: Vec<DataCell> = Vec::new();
        let mut splatted: Vec<DataCell> = Vec::new();

        for cell in cells {
            if num_instances > 1 && cell.num_instances() == 1 {
                splatted.push(cell);
            } else {
                instanced.push(cell);
            }
        }

        // NOTE: The timepoint is irrelevant, the `RecordingStream` will overwrite it using its
        // internal clock.
        let timepoint = TimePoint::timeless();

        // TODO(#1893): unsplit splats once new data cells are in
        let splatted = if splatted.is_empty() {
            None
        } else {
            splatted.push(DataCell::from_native([InstanceKey::SPLAT]));
            Some(DataRow::from_cells(
                row_id,
                timepoint.clone(),
                ent_path.clone(),
                1,
                splatted,
            )?)
        };

        let instanced = if instanced.is_empty() {
            None
        } else {
            Some(DataRow::from_cells(
                row_id.incremented_by(1), // we need a unique RowId from what is used for the splatted data
                timepoint,
                ent_path,
                num_instances as _,
                instanced,
            )?)
        };

        if let Some(splatted) = splatted {
            self.record_row(splatted, !timeless);
        }

        // Always the primary component last so range-based queries will include the other data.
        // Since the primary component can't be splatted it must be in here, see(#1215).
        if let Some(instanced) = instanced {
            self.record_row(instanced, !timeless);
        }

        Ok(())
    }
}

#[allow(clippy::needless_pass_by_value)]
fn forwarding_thread(
    info: StoreInfo,
    mut sink: Box<dyn LogSink>,
    cmds_rx: Receiver<Command>,
    tables: Receiver<DataTable>,
    on_release: Option<ArrowChunkReleaseCallback>,
) {
    /// Returns `true` to indicate that processing can continue; i.e. `false` means immediate
    /// shutdown.
    fn handle_cmd(info: &StoreInfo, cmd: Command, sink: &mut Box<dyn LogSink>) -> bool {
        match cmd {
            Command::RecordMsg(msg) => {
                sink.send(msg);
            }
            Command::SwapSink(new_sink) => {
                re_log::trace!("Swapping sink…");
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
                        rec_id = %info.store_id,
                        "setting recording info",
                    );
                    new_sink.send(
                        re_log_types::SetStoreInfo {
                            row_id: re_log_types::RowId::new(),
                            info: info.clone(),
                        }
                        .into(),
                    );
                    new_sink.send_all(backlog);
                }

                *sink = new_sink;
            }
            Command::Flush(oneshot) => {
                re_log::trace!("Flushing…");
                // Flush the underlying sink if possible.
                sink.drop_if_disconnected();
                sink.flush_blocking();
                drop(oneshot); // signals the oneshot
            }
            Command::PopPendingTables => {
                // Wake up and skip the current iteration so that we can drain all pending tables
                // before handling the next command.
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
            let mut arrow_msg = match table.to_arrow_msg() {
                Ok(table) => table,
                Err(err) => {
                    re_log::error!(%err,
                        "couldn't serialize table; data dropped (this is a bug in Rerun!)");
                    continue;
                }
            };
            arrow_msg.on_release = on_release.clone();
            sink.send(LogMsg::ArrowMsg(info.store_id.clone(), arrow_msg));
        }

        select! {
            recv(tables) -> res => {
                let Ok(table) = res else {
                    // The batcher is gone, which can only happen if the `RecordingStream` itself
                    // has been dropped.
                    re_log::trace!("Shutting down forwarding_thread: batcher is gone");
                    break;
                };
                let mut arrow_msg = match table.to_arrow_msg() {
                    Ok(table) => table,
                    Err(err) => {
                        re_log::error!(%err,
                            "couldn't serialize table; data dropped (this is a bug in Rerun!)");
                        continue;
                    }
                };
                arrow_msg.on_release = on_release.clone();
                sink.send(LogMsg::ArrowMsg(info.store_id.clone(), arrow_msg));
            }
            recv(cmds_rx) -> res => {
                let Ok(cmd) = res else {
                    // All command senders are gone, which can only happen if the
                    // `RecordingStream` itself has been dropped.
                    re_log::trace!("Shutting down forwarding_thread: all command senders are gone");
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

    /// The [`StoreInfo`] associated with this `RecordingStream`.
    #[inline]
    pub fn store_info(&self) -> Option<&StoreInfo> {
        (*self.inner).as_ref().map(|inner| &inner.info)
    }

    /// Determine whether a fork has happened since creating this `RecordingStream`. In general, this means our
    /// batcher/sink threads are gone and all data logged since the fork has been dropped.
    ///
    /// It is essential that [`crate::cleanup_if_forked_child`] be called after forking the process. SDK-implementations
    /// should do this during their initialization phase.
    #[inline]
    pub fn is_forked_child(&self) -> bool {
        (*self.inner)
            .as_ref()
            .map_or(false, |inner| inner.is_forked_child())
    }
}

impl RecordingStream {
    /// Records an arbitrary [`LogMsg`].
    #[inline]
    pub fn record_msg(&self, msg: LogMsg) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to record_msg() ignored");
            return;
        };

        // NOTE: Internal channels can never be closed outside of the `Drop` impl, this send cannot
        // fail.

        this.cmds_tx.send(Command::RecordMsg(msg)).ok();
        this.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Records a single [`DataRow`].
    ///
    /// If `inject_time` is set to `true`, the row's timestamp data will be overridden using the
    /// [`RecordingStream`]'s internal clock.
    ///
    /// Internally, incoming [`DataRow`]s are automatically coalesced into larger [`DataTable`]s to
    /// optimize for transport.
    #[inline]
    pub fn record_row(&self, mut row: DataRow, inject_time: bool) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to record_row() ignored");
            return;
        };

        // TODO(#2074): Adding a timeline to something timeless would suddenly make it not
        // timeless… so for now it cannot even have a tick :/
        //
        // NOTE: We're incrementing the current tick still.
        let tick = this.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if inject_time {
            // Get the current time on all timelines, for the current recording, on the current
            // thread…
            let mut now = self.now();
            // ...and then also inject the current recording tick into it.
            now.insert(Timeline::log_tick(), tick.into());

            // Inject all these times into the row, overriding conflicting times, if any.
            for (timeline, time) in now {
                row.timepoint.insert(timeline, time);
            }
        }

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
            re_log::warn_once!("Recording disabled - call to set_sink() ignored");
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
        re_log::trace!("Waiting for sink swap to complete…");
        let (cmd, oneshot) = Command::flush();
        this.cmds_tx.send(cmd).ok();
        oneshot.recv().ok();
        re_log::trace!("Sink swap completed.");
    }

    /// Initiates a flush of the pipeline and returns immediately.
    ///
    /// This does **not** wait for the flush to propagate (see [`Self::flush_blocking`]).
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    pub fn flush_async(&self) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to flush_async() ignored");
            return;
        };

        // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
        // are safe.

        // 1. Synchronously flush the batcher down the table channel
        //
        // NOTE: This _has_ to be done synchronously as we need to be guaranteed that all tables
        // are ready to be drained by the time this call returns.
        // It cannot block indefinitely and is fairly fast as it only requires compute (no I/O).
        this.batcher.flush_blocking();

        // 2. Drain all pending tables from the batcher's channel _before_ any other future command
        this.cmds_tx.send(Command::PopPendingTables).ok();

        // 3. Asynchronously flush everything down the sink
        let (cmd, _) = Command::flush();
        this.cmds_tx.send(cmd).ok();
    }

    /// Initiates a flush the batching pipeline and waits for it to propagate.
    ///
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    pub fn flush_blocking(&self) {
        if self.is_forked_child() {
            re_log::error_once!("Fork detected during flush. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK.");
            return;
        }

        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to flush_blocking() ignored");
            return;
        };

        // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
        // are safe.

        // 1. Flush the batcher down the table channel
        this.batcher.flush_blocking();

        // 2. Drain all pending tables from the batcher's channel _before_ any other future command
        this.cmds_tx.send(Command::PopPendingTables).ok();

        // 3. Wait for all tables to have been forwarded down the sink
        let (cmd, oneshot) = Command::flush();
        this.cmds_tx.send(cmd).ok();
        oneshot.recv().ok();
    }
}

impl RecordingStream {
    /// Swaps the underlying sink for a [`crate::log_sink::TcpSink`] sink pre-configured to use
    /// the specified address.
    ///
    /// See also [`Self::connect_opts`] if you wish to configure the TCP connection.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn connect(&self) {
        self.connect_opts(crate::default_server_addr(), crate::default_flush_timeout());
    }

    /// Swaps the underlying sink for a [`crate::log_sink::TcpSink`] sink pre-configured to use
    /// the specified address.
    ///
    /// `flush_timeout` is the minimum time the [`TcpSink`][`crate::log_sink::TcpSink`] will
    /// wait during a flush before potentially dropping data.  Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn connect_opts(
        &self,
        addr: std::net::SocketAddr,
        flush_timeout: Option<std::time::Duration>,
    ) {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new TcpSink since _RERUN_FORCE_SINK is set");
            return;
        }

        self.set_sink(Box::new(crate::log_sink::TcpSink::new(addr, flush_timeout)));
    }

    /// Spawns a new Rerun Viewer process from an executable available in PATH, then swaps the
    /// underlying sink for a [`crate::log_sink::TcpSink`] sink pre-configured to send data to that
    /// new process.
    ///
    /// If a Rerun Viewer is already listening on this TCP port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// See also [`Self::spawn_opts`] if you wish to configure the behavior of thew Rerun process
    /// as well as the underlying TCP connection.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn spawn(&self) -> RecordingStreamResult<()> {
        self.spawn_opts(&Default::default(), crate::default_flush_timeout())
    }

    /// Spawns a new Rerun Viewer process from an executable available in PATH, then swaps the
    /// underlying sink for a [`crate::log_sink::TcpSink`] sink pre-configured to send data to that
    /// new process.
    ///
    /// If a Rerun Viewer is already listening on this TCP port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// The behavior of the spawned Viewer can be configured via `opts`.
    /// If you're fine with the default behavior, refer to the simpler [`Self::spawn`].
    ///
    /// `flush_timeout` is the minimum time the [`TcpSink`][`crate::log_sink::TcpSink`] will
    /// wait during a flush before potentially dropping data.  Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn spawn_opts(
        &self,
        opts: &crate::SpawnOptions,
        flush_timeout: Option<std::time::Duration>,
    ) -> RecordingStreamResult<()> {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new TcpSink since _RERUN_FORCE_SINK is set");
            return Ok(());
        }

        spawn(opts)?;

        self.connect_opts(opts.connect_addr(), flush_timeout);

        Ok(())
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

        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new memory sink since _RERUN_FORCE_SINK is set");
            return buffer;
        }

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
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new file since _RERUN_FORCE_SINK is set");
            return Ok(());
        }

        let sink = crate::sink::FileSink::new(path)?;
        self.set_sink(Box::new(sink));

        Ok(())
    }

    /// Swaps the underlying sink for a [`crate::sink::FileSink`] pointed at stdout.
    ///
    /// If there isn't any listener at the other end of the pipe, the [`RecordingStream`] will
    /// default back to `buffered` mode, in order not to break the user's terminal.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn stdout(&self) -> Result<(), crate::sink::FileSinkError> {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new file since _RERUN_FORCE_SINK is set");
            return Ok(());
        }

        let is_stdout_listening = !atty::is(atty::Stream::Stdout);
        if !is_stdout_listening {
            self.set_sink(Box::new(crate::log_sink::BufferedSink::new()));
            return Ok(());
        }

        let sink = crate::sink::FileSink::stdout()?;
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

impl fmt::Debug for RecordingStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.inner {
            Some(RecordingStreamInner {
                // This pattern match prevents _accidentally_ omitting data from the debug output
                // when new fields are added.
                info,
                tick,
                cmds_tx: _,
                batcher: _,
                batcher_to_sink_handle: _,
                pid_at_creation,
            }) => f
                .debug_struct("RecordingStream")
                .field("info", &info)
                .field("tick", &tick)
                .field("pid_at_creation", &pid_at_creation)
                .finish_non_exhaustive(),
            None => write!(f, "RecordingStream {{ disabled }}"),
        }
    }
}

/// Helper to deduplicate spawn logic across [`RecordingStreamBuilder`] & [`RecordingStream`].
fn spawn(opts: &crate::SpawnOptions) -> RecordingStreamResult<()> {
    use std::{net::TcpStream, time::Duration};

    let connect_addr = opts.connect_addr();

    // TODO(#4019): application-level handshake
    if TcpStream::connect_timeout(&connect_addr, Duration::from_secs(1)).is_ok() {
        re_log::info!(
            addr = %opts.listen_addr(),
            "A process is already listening at this address. Trying to connect instead."
        );
    } else {
        crate::spawn(opts)?;

        // Give the newly spawned Rerun Viewer some time to bind.
        //
        // NOTE: The timeout only covers the TCP handshake: if no process is bound to that address
        // at all, the connection will fail immediately, irrelevant of the timeout configuration.
        // For that reason we use an extra loop.
        for _ in 0..5 {
            if TcpStream::connect_timeout(&connect_addr, Duration::from_secs(1)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    Ok(())
}

// --- Stateful time ---

/// Thread-local data.
#[derive(Default)]
struct ThreadInfo {
    /// The current time per-thread per-recording, which can be set by users.
    timepoints: HashMap<StoreId, TimePoint>,
}

impl ThreadInfo {
    fn thread_now(rid: &StoreId) -> TimePoint {
        Self::with(|ti| ti.now(rid))
    }

    fn set_thread_time(rid: &StoreId, timeline: Timeline, time_int: TimeInt) {
        Self::with(|ti| ti.set_time(rid, timeline, time_int));
    }

    fn unset_thread_time(rid: &StoreId, timeline: Timeline) {
        Self::with(|ti| ti.unset_time(rid, timeline));
    }

    fn reset_thread_time(rid: &StoreId) {
        Self::with(|ti| ti.reset_time(rid));
    }

    /// Get access to the thread-local [`ThreadInfo`].
    fn with<R>(f: impl FnOnce(&mut ThreadInfo) -> R) -> R {
        use std::cell::RefCell;
        thread_local! {
            static THREAD_INFO: RefCell<Option<ThreadInfo>> = RefCell::new(None);
        }

        THREAD_INFO.with(|thread_info| {
            let mut thread_info = thread_info.borrow_mut();
            let thread_info = thread_info.get_or_insert_with(ThreadInfo::default);
            f(thread_info)
        })
    }

    fn now(&self, rid: &StoreId) -> TimePoint {
        let mut timepoint = self.timepoints.get(rid).cloned().unwrap_or_default();
        timepoint.insert(Timeline::log_time(), Time::now().into());
        timepoint
    }

    fn set_time(&mut self, rid: &StoreId, timeline: Timeline, time_int: TimeInt) {
        self.timepoints
            .entry(rid.clone())
            .or_default()
            .insert(timeline, time_int);
    }

    fn unset_time(&mut self, rid: &StoreId, timeline: Timeline) {
        if let Some(timepoint) = self.timepoints.get_mut(rid) {
            timepoint.remove(&timeline);
        }
    }

    fn reset_time(&mut self, rid: &StoreId) {
        if let Some(timepoint) = self.timepoints.get_mut(rid) {
            *timepoint = TimePoint::default();
        }
    }
}

impl RecordingStream {
    /// Returns the current time of the recording on the current thread.
    pub fn now(&self) -> TimePoint {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to now() ignored");
            return TimePoint::default();
        };

        ThreadInfo::thread_now(&this.info.store_id)
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the time setting methods.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_time_seconds`]
    /// - [`Self::set_time_nanos`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_timepoint(&self, timepoint: impl Into<TimePoint>) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to set_time_sequence() ignored");
            return;
        };

        let timepoint = timepoint.into();

        for (timeline, time) in timepoint {
            ThreadInfo::set_thread_time(&this.info.store_id, timeline, time);
        }
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the time setting methods.
    ///
    /// For example: `rec.set_time_sequence("frame_nr", frame_nr)`.
    /// You can remove a timeline again using `rec.disable_timeline("frame_nr")`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_time_seconds`]
    /// - [`Self::set_time_nanos`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_time_sequence(&self, timeline: impl Into<TimelineName>, sequence: impl Into<i64>) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to set_time_sequence() ignored");
            return;
        };

        ThreadInfo::set_thread_time(
            &this.info.store_id,
            Timeline::new(timeline, TimeType::Sequence),
            sequence.into().into(),
        );
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the time setting methods.
    ///
    /// For example: `rec.set_time_seconds("sim_time", sim_time_secs)`.
    /// You can remove a timeline again using `rec.disable_timeline("sim_time")`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_time_nanos`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_time_seconds(&self, timeline: impl Into<TimelineName>, seconds: impl Into<f64>) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to set_time_seconds() ignored");
            return;
        };

        ThreadInfo::set_thread_time(
            &this.info.store_id,
            Timeline::new(timeline, TimeType::Time),
            Time::from_seconds_since_epoch(seconds.into()).into(),
        );
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the time setting methods.
    ///
    /// For example: `rec.set_time_nanos("sim_time", sim_time_nanos)`.
    /// You can remove a timeline again using `rec.disable_timeline("sim_time")`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_time_seconds`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_time_nanos(&self, timeline: impl Into<TimelineName>, ns: impl Into<i64>) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to set_time_nanos() ignored");
            return;
        };

        ThreadInfo::set_thread_time(
            &this.info.store_id,
            Timeline::new(timeline, TimeType::Time),
            Time::from_ns_since_epoch(ns.into()).into(),
        );
    }

    /// Clears out the current time of the recording for the specified timeline, for the
    /// current calling thread.
    ///
    /// For example: `rec.disable_timeline("frame")`, `rec.disable_timeline("sim_time")`.
    ///
    /// See also:
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_time_seconds`]
    /// - [`Self::set_time_nanos`]
    /// - [`Self::reset_time`]
    pub fn disable_timeline(&self, timeline: impl Into<TimelineName>) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to disable_timeline() ignored");
            return;
        };

        let timeline = timeline.into();
        ThreadInfo::unset_thread_time(&this.info.store_id, Timeline::new_sequence(timeline));
        ThreadInfo::unset_thread_time(&this.info.store_id, Timeline::new_temporal(timeline));
    }

    /// Clears out the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the time setting methods.
    ///
    /// For example: `rec.reset_time()`.
    ///
    /// See also:
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_time_seconds`]
    /// - [`Self::set_time_nanos`]
    /// - [`Self::disable_timeline`]
    pub fn reset_time(&self) {
        let Some(this) = &*self.inner else {
            re_log::warn_once!("Recording disabled - call to reset_time() ignored");
            return;
        };

        ThreadInfo::reset_thread_time(&this.info.store_id);
    }
}

// ---

#[cfg(test)]
mod tests {
    use re_log_types::{DataTable, RowId};

    use super::*;

    #[test]
    fn impl_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RecordingStream>();
    }

    #[test]
    fn never_flush() {
        let rec = RecordingStreamBuilder::new("rerun_example_never_flush")
            .enabled(true)
            .batcher_config(DataTableBatcherConfig::NEVER)
            .buffered()
            .unwrap();

        let store_info = rec.store_info().cloned().unwrap();

        let mut table = DataTable::example(false);
        table.compute_all_size_bytes();
        for row in table.to_rows() {
            rec.record_row(row.unwrap(), false);
        }

        let storage = rec.memory();
        let mut msgs = {
            let mut msgs = storage.take();
            msgs.reverse();
            msgs
        };

        // First message should be a set_store_info resulting from the original sink swap to
        // buffered mode.
        match msgs.pop().unwrap() {
            LogMsg::SetStoreInfo(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            LogMsg::ArrowMsg { .. } => panic!("expected SetStoreInfo"),
        }

        // Second message should be a set_store_info resulting from the later sink swap from
        // buffered mode into in-memory mode.
        // This arrives _before_ the data itself since we're using manual flushing.
        match msgs.pop().unwrap() {
            LogMsg::SetStoreInfo(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            LogMsg::ArrowMsg { .. } => panic!("expected SetStoreInfo"),
        }

        // Third message is the batched table itself, which was sent as a result of the implicit
        // flush when swapping the underlying sink from buffered to in-memory.
        match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(store_info.store_id, rid);

                let mut got = DataTable::from_arrow_msg(&msg).unwrap();
                // TODO(#1760): we shouldn't have to (re)do this!
                got.compute_all_size_bytes();
                // NOTE: Override the resulting table's ID so they can be compared.
                got.table_id = table.table_id;

                similar_asserts::assert_eq!(table, got);
            }
            LogMsg::SetStoreInfo { .. } => panic!("expected ArrowMsg"),
        }

        // That's all.
        assert!(msgs.pop().is_none());
    }

    #[test]
    fn always_flush() {
        use itertools::Itertools as _;

        let rec = RecordingStreamBuilder::new("rerun_example_always_flush")
            .enabled(true)
            .batcher_config(DataTableBatcherConfig::ALWAYS)
            .buffered()
            .unwrap();

        let store_info = rec.store_info().cloned().unwrap();

        let mut table = DataTable::example(false);
        table.compute_all_size_bytes();
        for row in table.to_rows() {
            rec.record_row(row.unwrap(), false);
        }

        let storage = rec.memory();
        let mut msgs = {
            let mut msgs = storage.take();
            msgs.reverse();
            msgs
        };

        // First message should be a set_store_info resulting from the original sink swap to
        // buffered mode.
        match msgs.pop().unwrap() {
            LogMsg::SetStoreInfo(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            LogMsg::ArrowMsg { .. } => panic!("expected SetStoreInfo"),
        }

        // Second message should be a set_store_info resulting from the later sink swap from
        // buffered mode into in-memory mode.
        // This arrives _before_ the data itself since we're using manual flushing.
        match msgs.pop().unwrap() {
            LogMsg::SetStoreInfo(msg) => {
                assert!(msg.row_id != RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            LogMsg::ArrowMsg { .. } => panic!("expected SetStoreInfo"),
        }

        let mut rows = {
            let mut rows: Vec<_> = table.to_rows().try_collect().unwrap();
            rows.reverse();
            rows
        };

        let mut assert_next_row = || {
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(store_info.store_id, rid);

                    let mut got = DataTable::from_arrow_msg(&msg).unwrap();
                    // TODO(#1760): we shouldn't have to (re)do this!
                    got.compute_all_size_bytes();
                    // NOTE: Override the resulting table's ID so they can be compared.
                    got.table_id = table.table_id;

                    let expected = DataTable::from_rows(got.table_id, [rows.pop().unwrap()]);

                    similar_asserts::assert_eq!(expected, got);
                }
                LogMsg::SetStoreInfo { .. } => panic!("expected ArrowMsg"),
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

    #[test]
    fn flush_hierarchy() {
        let (rec, storage) = RecordingStreamBuilder::new("rerun_example_flush_hierarchy")
            .enabled(true)
            .batcher_config(DataTableBatcherConfig::NEVER)
            .memory()
            .unwrap();

        let store_info = rec.store_info().cloned().unwrap();

        let mut table = DataTable::example(false);
        table.compute_all_size_bytes();
        for row in table.to_rows() {
            rec.record_row(row.unwrap(), false);
        }

        {
            let mut msgs = {
                let mut msgs = storage.take();
                msgs.reverse();
                msgs
            };

            // First message should be a set_store_info resulting from the original sink swap
            // to in-memory mode.
            match msgs.pop().unwrap() {
                LogMsg::SetStoreInfo(msg) => {
                    assert!(msg.row_id != RowId::ZERO);
                    similar_asserts::assert_eq!(store_info, msg.info);
                }
                LogMsg::ArrowMsg { .. } => panic!("expected SetStoreInfo"),
            }

            // MemorySinkStorage transparently handles flushing during `take()`!

            // The batched table itself, which was sent as a result of the explicit flush above.
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(store_info.store_id, rid);

                    let mut got = DataTable::from_arrow_msg(&msg).unwrap();
                    // TODO(#1760): we shouldn't have to (re)do this!
                    got.compute_all_size_bytes();
                    // NOTE: Override the resulting table's ID so they can be compared.
                    got.table_id = table.table_id;

                    similar_asserts::assert_eq!(table, got);
                }
                LogMsg::SetStoreInfo { .. } => panic!("expected ArrowMsg"),
            }

            // That's all.
            assert!(msgs.pop().is_none());
        }
    }

    #[test]
    fn disabled() {
        let (rec, storage) = RecordingStreamBuilder::new("rerun_example_disabled")
            .enabled(false)
            .batcher_config(DataTableBatcherConfig::ALWAYS)
            .memory()
            .unwrap();

        let mut table = DataTable::example(false);
        table.compute_all_size_bytes();
        for row in table.to_rows() {
            rec.record_row(row.unwrap(), false);
        }

        let mut msgs = {
            let mut msgs = storage.take();
            msgs.reverse();
            msgs
        };

        // That's all.
        assert!(msgs.pop().is_none());
    }

    #[test]
    fn test_set_thread_local() {
        // Regression-test for https://github.com/rerun-io/rerun/issues/2889
        std::thread::Builder::new()
            .name("test_thead".to_owned())
            .spawn(|| {
                let stream = RecordingStreamBuilder::new("rerun_example_test")
                    .buffered()
                    .unwrap();
                RecordingStream::set_thread_local(StoreKind::Recording, Some(stream));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
