use std::fmt;
use std::io::IsTerminal as _;
use std::sync::Weak;
use std::sync::{atomic::AtomicI64, Arc};
use std::time::Duration;

use ahash::HashMap;
use crossbeam::channel::{Receiver, Sender};
use itertools::Either;
use nohash_hasher::IntMap;
use parking_lot::Mutex;

use re_chunk::{
    Chunk, ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError, ChunkComponents, ChunkError,
    ChunkId, PendingRow, RowId, TimeColumn,
};
use re_log_types::{
    ApplicationId, ArrowRecordBatchReleaseCallback, BlueprintActivationCommand, EntityPath, LogMsg,
    StoreId, StoreInfo, StoreKind, StoreSource, TimeCell, TimeInt, TimePoint, Timeline,
    TimelineName,
};
use re_types::archetypes::RecordingProperties;
use re_types::components::Timestamp;
use re_types::{AsComponents, SerializationError, SerializedComponentColumn};

#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;

use crate::binary_stream_sink::BinaryStreamStorage;
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
pub fn forced_sink_path() -> Option<String> {
    std::env::var(ENV_FORCE_SAVE).ok()
}

/// Errors that can occur when creating/manipulating a [`RecordingStream`].
#[derive(thiserror::Error, Debug)]
pub enum RecordingStreamError {
    /// Error within the underlying file sink.
    #[error("Failed to create the underlying file sink: {0}")]
    FileSink(#[from] re_log_encoding::FileSinkError),

    /// Error within the underlying chunk batcher.
    #[error("Failed to convert data to a valid chunk: {0}")]
    Chunk(#[from] ChunkError),

    /// Error within the underlying chunk batcher.
    #[error("Failed to spawn the underlying batcher: {0}")]
    ChunkBatcher(#[from] ChunkBatcherError),

    /// Error within the underlying serializer.
    #[error("Failed to serialize component data: {0}")]
    Serialization(#[from] SerializationError),

    /// Error spawning one of the background threads.
    #[error("Failed to spawn background thread '{name}': {err}")]
    SpawnThread {
        /// Name of the thread
        name: String,

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

    /// An error occurred while attempting to use a [`re_data_loader::DataLoader`].
    #[cfg(feature = "data_loaders")]
    #[error(transparent)]
    DataLoaderError(#[from] re_data_loader::DataLoaderError),

    /// Invalid gRPC server address.
    #[error(transparent)]
    UriError(#[from] re_uri::Error),

    /// Invalid endpoint
    #[error("not a `/proxy` endpoint")]
    NotAProxyEndpoint,
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
///
/// Automatically sends a [`Chunk`] with the default [`RecordingProperties`] to
/// the sink, unless an explicit `recording_id` is set via [`RecordingStreamBuilder::recording_id`].
#[derive(Debug)]
pub struct RecordingStreamBuilder {
    application_id: ApplicationId,
    store_kind: StoreKind,
    store_id: Option<StoreId>,
    store_source: Option<StoreSource>,

    default_enabled: bool,
    enabled: Option<bool>,

    batcher_config: Option<ChunkBatcherConfig>,

    // Optional user-defined recording properties.
    properties: Option<RecordingProperties>,
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

        Self {
            application_id,
            store_kind: StoreKind::Recording,
            store_id: None,
            store_source: None,

            default_enabled: true,
            enabled: None,

            batcher_config: None,

            properties: Some(
                RecordingProperties::new().with_start_time(re_types::components::Timestamp::now()),
            ),
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
    ///
    /// When explicitly setting a `RecordingId`, the initial chunk that contains the recording
    /// properties will not be sent.
    #[inline]
    pub fn recording_id(mut self, recording_id: impl Into<String>) -> Self {
        self.store_id = Some(StoreId::from_string(
            StoreKind::Recording,
            recording_id.into(),
        ));
        self.disable_properties()
    }

    /// Sets an optional name for the recording.
    #[inline]
    pub fn recording_name(mut self, name: impl Into<String>) -> Self {
        self.properties = if let Some(props) = self.properties.take() {
            Some(props.with_name(name.into()))
        } else {
            Some(RecordingProperties::new().with_name(name.into()))
        };
        self
    }

    /// Sets an optional name for the recording.
    #[inline]
    pub fn recording_started(mut self, started: impl Into<Timestamp>) -> Self {
        self.properties = if let Some(props) = self.properties.take() {
            Some(props.with_start_time(started))
        } else {
            Some(RecordingProperties::new().with_start_time(started))
        };
        self
    }

    /// Disables sending the [`RecordingProperties`] chunk.
    #[inline]
    pub fn disable_properties(mut self) -> Self {
        self.properties = None;
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
        self.store_kind = store_id.kind;
        self.store_id = Some(store_id);
        self
    }

    /// Specifies the configuration of the internal data batching mechanism.
    ///
    /// See [`ChunkBatcher`] & [`ChunkBatcherConfig`] for more information.
    #[inline]
    pub fn batcher_config(mut self, config: ChunkBatcherConfig) -> Self {
        self.batcher_config = Some(config);
        self
    }

    #[doc(hidden)]
    #[inline]
    pub fn store_source(mut self, store_source: StoreSource) -> Self {
        self.store_source = Some(store_source);
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
        let (enabled, store_info, properties, batcher_config) = self.into_args();
        if enabled {
            RecordingStream::new(
                store_info,
                properties,
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
        let (enabled, store_info, properties, batcher_config) = self.into_args();
        let rec = if enabled {
            RecordingStream::new(
                store_info,
                properties,
                batcher_config,
                Box::new(crate::log_sink::BufferedSink::new()),
            )
        } else {
            re_log::debug!("Rerun disabled - call to memory() ignored");
            Ok(RecordingStream::disabled())
        }?;

        let sink = crate::log_sink::MemorySink::new(rec.clone());
        let storage = sink.buffer();
        // Using set_sink here is necessary because the MemorySink needs to know
        // it's own RecordingStream, which means we can't use `new` above.
        // This has the downside of a bit of creation overhead and an extra StoreInfo
        // message being sent to the sink.
        // TODO(jleibs): Figure out a cleaner way to handle this.
        rec.set_sink(Box::new(sink));
        Ok((rec, storage))
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// remote Rerun instance.
    ///
    /// See also [`Self::connect_grpc_opts`] if you wish to configure the connection.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app").connect_grpc()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn connect_grpc(self) -> RecordingStreamResult<RecordingStream> {
        self.connect_grpc_opts(
            format!(
                "rerun+http://127.0.0.1:{}/proxy",
                re_grpc_server::DEFAULT_SERVER_PORT
            ),
            crate::default_flush_timeout(),
        )
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// remote Rerun instance.
    ///
    /// `flush_timeout` is the minimum time the [`GrpcSink`][`crate::log_sink::GrpcSink`] will
    /// wait during a flush before potentially dropping data. Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app")
    ///     .connect_grpc_opts("rerun+http://127.0.0.1:9876/proxy", re_sdk::default_flush_timeout())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn connect_grpc_opts(
        self,
        url: impl Into<String>,
        flush_timeout: Option<Duration>,
    ) -> RecordingStreamResult<RecordingStream> {
        let (enabled, store_info, properties, batcher_config) = self.into_args();
        if enabled {
            let url: String = url.into();
            let re_uri::RedapUri::Proxy(endpoint) = re_uri::RedapUri::try_from(url.as_str())?
            else {
                return Err(RecordingStreamError::NotAProxyEndpoint);
            };

            RecordingStream::new(
                store_info,
                properties,
                batcher_config,
                Box::new(crate::log_sink::GrpcSink::new(endpoint, flush_timeout)),
            )
        } else {
            re_log::debug!("Rerun disabled - call to connect() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to an
    /// RRD file on disk.
    ///
    /// The Rerun Viewer is able to read continuously from the resulting rrd file while it is being written.
    /// However, depending on your OS and configuration, changes may not be immediately visible due to file caching.
    /// This is a common issue on Windows and (to a lesser extent) on `MacOS`.
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
        let (enabled, store_info, properties, batcher_config) = self.into_args();

        if enabled {
            RecordingStream::new(
                store_info,
                properties,
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
        if std::io::stdout().is_terminal() {
            re_log::debug!("Ignored call to stdout() because stdout is a terminal");
            return self.buffered();
        }

        let (enabled, store_info, properties, batcher_config) = self.into_args();

        if enabled {
            RecordingStream::new(
                store_info,
                properties,
                batcher_config,
                Box::new(crate::sink::FileSink::stdout()?),
            )
        } else {
            re_log::debug!("Rerun disabled - call to stdout() ignored");
            Ok(RecordingStream::disabled())
        }
    }

    /// Spawns a new Rerun Viewer process from an executable available in PATH, then creates a new
    /// [`RecordingStream`] that is pre-configured to stream the data through to that viewer over gRPC.
    ///
    /// If a Rerun Viewer is already listening on this port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// See also [`Self::spawn_opts`] if you wish to configure the behavior of thew Rerun process
    /// as well as the underlying connection.
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
    /// [`RecordingStream`] that is pre-configured to stream the data through to that viewer over gRPC.
    ///
    /// If a Rerun Viewer is already listening on this port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// The behavior of the spawned Viewer can be configured via `opts`.
    /// If you're fine with the default behavior, refer to the simpler [`Self::spawn`].
    ///
    /// `flush_timeout` is the minimum time the [`GrpcSink`][`crate::log_sink::GrpcSink`] will
    /// wait during a flush before potentially dropping data. Note: Passing `None` here can cause a
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
        flush_timeout: Option<Duration>,
    ) -> RecordingStreamResult<RecordingStream> {
        if !self.is_enabled() {
            re_log::debug!("Rerun disabled - call to spawn() ignored");
            return Ok(RecordingStream::disabled());
        }

        let url = format!("rerun+http://{}/proxy", opts.connect_addr());

        // NOTE: If `_RERUN_TEST_FORCE_SAVE` is set, all recording streams will write to disk no matter
        // what, thus spawning a viewer is pointless (and probably not intended).
        if forced_sink_path().is_some() {
            return self.connect_grpc_opts(url, flush_timeout);
        }

        crate::spawn(opts)?;

        self.connect_grpc_opts(url, flush_timeout)
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// web-based Rerun viewer via WebSockets.
    ///
    /// If the `open_browser` argument is `true`, your default browser will be opened with a
    /// connected web-viewer.
    ///
    /// If not, you can connect to this server using the `rerun` binary (`cargo install rerun-cli --locked`).
    ///
    /// ## Details
    /// This method will spawn two servers: one HTTPS server serving the Rerun Web Viewer `.html` and `.wasm` files,
    /// and then one WebSocket server that streams the log data to the web viewer (or to a native viewer, or to multiple viewers).
    ///
    /// The WebSocket server will buffer all log data in memory so that late connecting viewers will get all the data.
    /// You can limit the amount of data buffered by the WebSocket server with the `server_memory_limit` argument.
    /// Once reached, the earliest logged data will be dropped.
    /// Note that this means that static data may be dropped if logged early (see <https://github.com/rerun-io/rerun/issues/5531>).
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app")
    ///     .serve("0.0.0.0",
    ///            Default::default(),
    ///            Default::default(),
    ///            re_sdk::MemoryLimit::from_fraction_of_total(0.25),
    ///            true)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    //
    // # TODO(#5531): keep static data around.
    #[cfg(feature = "web_viewer")]
    #[deprecated(since = "0.20.0", note = "use serve_web() instead")]
    pub fn serve(
        self,
        bind_ip: &str,
        web_port: WebViewerServerPort,
        grpc_port: u16,
        server_memory_limit: re_memory::MemoryLimit,
        open_browser: bool,
    ) -> RecordingStreamResult<RecordingStream> {
        self.serve_web(
            bind_ip,
            web_port,
            grpc_port,
            server_memory_limit,
            open_browser,
        )
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// web-based Rerun viewer via WebSockets.
    ///
    /// If the `open_browser` argument is `true`, your default browser will be opened with a
    /// connected web-viewer.
    ///
    /// If not, you can connect to this server using the `rerun` binary (`cargo install rerun-cli --locked`).
    ///
    /// ## Details
    /// This method will spawn two servers: one HTTPS server serving the Rerun Web Viewer `.html` and `.wasm` files,
    /// and then one WebSocket server that streams the log data to the web viewer (or to a native viewer, or to multiple viewers).
    ///
    /// The WebSocket server will buffer all log data in memory so that late connecting viewers will get all the data.
    /// You can limit the amount of data buffered by the WebSocket server with the `server_memory_limit` argument.
    /// Once reached, the earliest logged data will be dropped.
    /// Note that this means that static data may be dropped if logged early (see <https://github.com/rerun-io/rerun/issues/5531>).
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app")
    ///     .serve_web("0.0.0.0",
    ///                Default::default(),
    ///                Default::default(),
    ///                re_sdk::MemoryLimit::from_fraction_of_total(0.25),
    ///                true)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    //
    // # TODO(#5531): keep static data around.
    #[cfg(feature = "web_viewer")]
    pub fn serve_web(
        self,
        bind_ip: &str,
        web_port: WebViewerServerPort,
        grpc_port: u16,
        server_memory_limit: re_memory::MemoryLimit,
        open_browser: bool,
    ) -> RecordingStreamResult<RecordingStream> {
        let (enabled, store_info, properties, batcher_config) = self.into_args();
        if enabled {
            let sink = crate::web_viewer::new_sink(
                open_browser,
                bind_ip,
                web_port,
                grpc_port,
                server_memory_limit,
            )?;
            RecordingStream::new(store_info, properties, batcher_config, sink)
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
    pub fn into_args(
        self,
    ) -> (
        bool,
        StoreInfo,
        Option<RecordingProperties>,
        ChunkBatcherConfig,
    ) {
        let enabled = self.is_enabled();

        let Self {
            application_id,
            store_kind,
            store_id,
            store_source,
            default_enabled: _,
            enabled: _,
            batcher_config,
            properties,
        } = self;

        let store_id = store_id.unwrap_or(StoreId::random(store_kind));
        let store_source = store_source.unwrap_or_else(|| StoreSource::RustSdk {
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
        });

        let store_info = StoreInfo {
            application_id,
            store_id,
            cloned_from: None,
            store_source,
            store_version: Some(re_build_info::CrateVersion::LOCAL),
        };

        let batcher_config =
            batcher_config.unwrap_or_else(|| match ChunkBatcherConfig::from_env() {
                Ok(config) => config,
                Err(err) => {
                    re_log::error!("Failed to parse ChunkBatcherConfig from env: {}", err);
                    ChunkBatcherConfig::default()
                }
            });

        (enabled, store_info, properties, batcher_config)
    }

    /// Internal check for whether or not logging is enabled using explicit/default settings & env var.
    fn is_enabled(&self) -> bool {
        self.enabled
            .unwrap_or_else(|| crate::decide_logging_enabled(self.default_enabled))
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
/// ([`RecordingStream::connect_grpc`], [`RecordingStream::memory`],
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
/// This means that e.g. flushing the pipeline ([`Self::flush_blocking`]) guarantees that all previous data sent by the calling thread
/// has been recorded and (if applicable) flushed to the underlying OS-managed file descriptor,
/// but other threads may still have data in flight.
///
/// ## Shutdown
///
/// The [`RecordingStream`] can only be shutdown by dropping all instances of it, at which point
/// it will automatically take care of flushing any pending data that might remain in the pipeline.
///
/// Shutting down cannot ever block.
#[derive(Clone)]
pub struct RecordingStream {
    inner: Either<Arc<Option<RecordingStreamInner>>, Weak<Option<RecordingStreamInner>>>,
}

impl RecordingStream {
    /// Passes a reference to the [`RecordingStreamInner`], if it exists.
    ///
    /// This works whether the underlying stream is strong or weak.
    #[inline]
    fn with<F: FnOnce(&RecordingStreamInner) -> R, R>(&self, f: F) -> Option<R> {
        use std::ops::Deref as _;
        match &self.inner {
            Either::Left(strong) => strong.deref().as_ref().map(f),
            Either::Right(weak) => weak
                .upgrade()
                .and_then(|strong| strong.deref().as_ref().map(f)),
        }
    }

    /// Clones the [`RecordingStream`] without incrementing the refcount.
    ///
    /// Useful e.g. if you want to make sure that a detached thread won't prevent the [`RecordingStream`]
    /// from flushing during shutdown.
    //
    // TODO(#5335): shutdown flushing behavior is too brittle.
    #[inline]
    pub fn clone_weak(&self) -> Self {
        Self {
            inner: match &self.inner {
                Either::Left(strong) => Either::Right(Arc::downgrade(strong)),
                Either::Right(weak) => Either::Right(Weak::clone(weak)),
            },
        }
    }
}

// TODO(#5335): shutdown flushing behavior is too brittle.
impl Drop for RecordingStream {
    #[inline]
    fn drop(&mut self) {
        // If this holds the last strong handle to the recording, make sure that all pending
        // `DataLoader` threads that were started from the SDK actually run to completion (they
        // all hold a weak handle to this very recording!).
        //
        // NOTE: It's very important to do so from the `Drop` implementation of `RecordingStream`
        // itself, because the dataloader threads -- by definition -- will have to send data into
        // this very recording, therefore we must make sure that at least one strong handle still lives
        // on until they are all finished.
        if let Either::Left(strong) = &mut self.inner {
            if Arc::strong_count(strong) == 1 {
                // Keep the recording alive until all dataloaders are finished.
                self.with(|inner| inner.wait_for_dataloaders());
            }
        }
    }
}

struct RecordingStreamInner {
    info: StoreInfo,
    properties: Option<RecordingProperties>,
    tick: AtomicI64,

    /// The one and only entrypoint into the pipeline: this is _never_ cloned nor publicly exposed,
    /// therefore the `Drop` implementation is guaranteed that no more data can come in while it's
    /// running.
    cmds_tx: Sender<Command>,

    batcher: ChunkBatcher,
    batcher_to_sink_handle: Option<std::thread::JoinHandle<()>>,

    /// Keeps track of the top-level threads that were spawned in order to execute the `DataLoader`
    /// machinery in the context of this `RecordingStream`.
    ///
    /// See [`RecordingStream::log_file_from_path`] and [`RecordingStream::log_file_from_contents`].
    dataloader_handles: Mutex<Vec<std::thread::JoinHandle<()>>>,

    pid_at_creation: u32,
}

impl fmt::Debug for RecordingStreamInner {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordingStreamInner")
            .field("info", &self.info.store_id)
            .finish()
    }
}

impl Drop for RecordingStreamInner {
    fn drop(&mut self) {
        if self.is_forked_child() {
            re_log::error_once!("Fork detected while dropping RecordingStreamInner. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK.");
            return;
        }

        self.wait_for_dataloaders();

        // NOTE: The command channel is private, if we're here, nothing is currently capable of
        // sending data down the pipeline.
        self.batcher.flush_blocking();
        self.cmds_tx.send(Command::PopPendingChunks).ok();
        self.cmds_tx.send(Command::Shutdown).ok();
        if let Some(handle) = self.batcher_to_sink_handle.take() {
            handle.join().ok();
        }
    }
}

impl RecordingStreamInner {
    fn new(
        info: StoreInfo,
        properties: Option<RecordingProperties>,
        batcher_config: ChunkBatcherConfig,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        let on_release = batcher_config.hooks.on_release.clone();
        let batcher = ChunkBatcher::new(batcher_config)?;

        {
            re_log::debug!(
                app_id = %info.application_id,
                rec_id = %info.store_id,
                "setting recording info",
            );
            sink.send(
                re_log_types::SetStoreInfo {
                    row_id: *RowId::new(),
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
                    move || forwarding_thread(info, sink, cmds_rx, batcher.chunks(), on_release)
                })
                .map_err(|err| RecordingStreamError::SpawnThread {
                    name: NAME.into(),
                    err,
                })?
        };

        if let Some(properties) = properties.as_ref() {
            // We pre-populate the batcher with a chunk the contains the recording
            // properties, so that these get automatically sent to the sink.

            re_log::debug!(properties = ?properties, "adding recording properties to batcher");

            let properties_chunk = Chunk::builder(EntityPath::recording_properties())
                .with_archetype(RowId::new(), TimePoint::default(), properties)
                .build()?;

            batcher.push_chunk(properties_chunk);
        }

        Ok(Self {
            info,
            properties,
            tick: AtomicI64::new(0),
            cmds_tx,
            batcher,
            batcher_to_sink_handle: Some(batcher_to_sink_handle),
            dataloader_handles: Mutex::new(Vec::new()),
            pid_at_creation: std::process::id(),
        })
    }

    #[inline]
    pub fn is_forked_child(&self) -> bool {
        self.pid_at_creation != std::process::id()
    }

    /// Make sure all pending top-level `DataLoader` threads that were started from the SDK run to completion.
    //
    // TODO(cmc): At some point we might want to make it configurable, though I cannot really
    // think of a use case where you'd want to drop those threads immediately upon
    // disconnection.
    fn wait_for_dataloaders(&self) {
        let dataloader_handles = std::mem::take(&mut *self.dataloader_handles.lock());
        for handle in dataloader_handles {
            handle.join().ok();
        }
    }
}

enum Command {
    RecordMsg(LogMsg),
    SwapSink(Box<dyn LogSink>),
    Flush(Sender<()>),
    PopPendingChunks,
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
        properties: Option<RecordingProperties>,
        batcher_config: ChunkBatcherConfig,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        let sink = (info.store_id.kind == StoreKind::Recording)
            .then(forced_sink_path)
            .flatten()
            .map_or(sink, |path| {
                re_log::info!("Forcing FileSink because of env-var {ENV_FORCE_SAVE}={path:?}");
                // `unwrap` is ok since this force sinks are only used in tests.
                Box::new(crate::sink::FileSink::new(path).unwrap()) as Box<dyn LogSink>
            });

        let stream =
            RecordingStreamInner::new(info, properties, batcher_config, sink).map(|inner| {
                Self {
                    inner: Either::Left(Arc::new(Some(inner))),
                }
            })?;

        Ok(stream)
    }

    /// Creates a new no-op [`RecordingStream`] that drops all logging messages, doesn't allocate
    /// any memory and doesn't spawn any threads.
    ///
    /// [`Self::is_enabled`] will return `false`.
    pub fn disabled() -> Self {
        Self {
            inner: Either::Left(Arc::new(None)),
        }
    }
}

impl RecordingStream {
    /// Log data to Rerun.
    ///
    /// This is the main entry point for logging data to rerun. It can be used to log anything
    /// that implements the [`AsComponents`], such as any [archetype](https://docs.rs/rerun/latest/rerun/archetypes/index.html)
    /// or individual [component](https://docs.rs/rerun/latest/rerun/components/index.html).
    ///
    /// The data will be timestamped automatically based on the [`RecordingStream`]'s internal clock.
    /// See [`RecordingStream::set_time_sequence`] etc for more information.
    ///
    /// The entity path can either be a string
    /// (with special characters escaped, split on unescaped slashes)
    /// or an [`EntityPath`] constructed with [`crate::entity_path`].
    /// See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
    ///
    /// See also: [`Self::log_static`] for logging static data.
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
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk/micro-batching
    /// [component bundle]: [`AsComponents`]
    #[inline]
    pub fn log<AS: ?Sized + AsComponents>(
        &self,
        ent_path: impl Into<EntityPath>,
        as_components: &AS,
    ) -> RecordingStreamResult<()> {
        self.log_with_static(ent_path, false, as_components)
    }

    /// Lower-level logging API to provide data spanning multiple timepoints.
    ///
    /// Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
    /// in a columnar form. The lengths of all of the [`TimeColumn`] and the component columns
    /// must match. All data that occurs at the same index across the different index/time and components
    /// arrays will act as a single logical row.
    ///
    /// Note that this API ignores any stateful index/time set on the log stream via the
    /// [`Self::set_time`]/[`Self::set_timepoint`]/[`Self::set_time_nanos`]/etc. APIs.
    /// Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.
    pub fn send_columns(
        &self,
        ent_path: impl Into<EntityPath>,
        indexes: impl IntoIterator<Item = TimeColumn>,
        columns: impl IntoIterator<Item = SerializedComponentColumn>,
    ) -> RecordingStreamResult<()> {
        let id = ChunkId::new();

        let indexes = indexes
            .into_iter()
            .map(|col| (*col.timeline().name(), col))
            .collect();

        let components: ChunkComponents = columns
            .into_iter()
            .map(|column| (column.descriptor, column.list_array))
            .collect();

        let chunk = Chunk::from_auto_row_ids(id, ent_path.into(), indexes, components)?;

        self.send_chunk(chunk);

        Ok(())
    }

    /// Log data to Rerun.
    ///
    /// It can be used to log anything
    /// that implements the [`AsComponents`], such as any [archetype](https://docs.rs/rerun/latest/rerun/archetypes/index.html)
    /// or individual [component](https://docs.rs/rerun/latest/rerun/components/index.html).
    ///
    /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
    /// any temporal data of the same type.
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
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk/micro-batching
    /// [component bundle]: [`AsComponents`]
    #[inline]
    pub fn log_static<AS: ?Sized + AsComponents>(
        &self,
        ent_path: impl Into<EntityPath>,
        as_components: &AS,
    ) -> RecordingStreamResult<()> {
        self.log_with_static(ent_path, true, as_components)
    }

    /// Logs the contents of a [component bundle] into Rerun.
    ///
    /// If `static_` is set to `true`, all timestamp data associated with this message will be
    /// dropped right before sending it to Rerun.
    /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
    /// any temporal data of the same type.
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
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk/micro-batching
    /// [component bundle]: [`AsComponents`]
    #[inline]
    pub fn log_with_static<AS: ?Sized + AsComponents>(
        &self,
        ent_path: impl Into<EntityPath>,
        static_: bool,
        as_components: &AS,
    ) -> RecordingStreamResult<()> {
        let row_id = RowId::new(); // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.
        self.log_serialized_batches_impl(
            row_id,
            ent_path,
            static_,
            as_components.as_serialized_batches(),
        )
    }

    /// Logs a set of [`SerializedComponentBatch`]es into Rerun.
    ///
    /// If `static_` is set to `true`, all timestamp data associated with this message will be
    /// dropped right before sending it to Rerun.
    /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
    /// any temporal data of the same type.
    ///
    /// Otherwise, the data will be timestamped automatically based on the [`RecordingStream`]'s
    /// internal clock.
    /// See `RecordingStream::set_time_*` family of methods for more information.
    ///
    /// The number of instances will be determined by the longest batch in the bundle.
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
    /// [SDK Micro Batching]: https://www.rerun.io/docs/reference/sdk/micro-batching
    ///
    /// [`SerializedComponentBatch`]: [re_types_core::SerializedComponentBatch]
    pub fn log_serialized_batches(
        &self,
        ent_path: impl Into<EntityPath>,
        static_: bool,
        comp_batches: impl IntoIterator<Item = re_types::SerializedComponentBatch>,
    ) -> RecordingStreamResult<()> {
        let row_id = RowId::new(); // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.
        self.log_serialized_batches_impl(row_id, ent_path, static_, comp_batches)
    }

    /// Sends a property to the recording.
    #[inline]
    pub fn send_property<AS: ?Sized + AsComponents>(
        &self,
        name: impl Into<String>,
        values: &AS,
    ) -> RecordingStreamResult<()> {
        let sub_path = EntityPath::from(name.into());
        self.log_static(EntityPath::properties().join(&sub_path), values)
    }

    /// Sends the name of the recording.
    #[inline]
    pub fn send_recording_name(&self, name: impl Into<String>) -> RecordingStreamResult<()> {
        let update = RecordingProperties::update_fields().with_name(name.into());
        self.log_static(EntityPath::recording_properties(), &update)
    }

    /// Sends the start time of the recording.
    #[inline]
    pub fn send_recording_start_time(
        &self,
        timestamp: impl Into<Timestamp>,
    ) -> RecordingStreamResult<()> {
        let update = RecordingProperties::update_fields().with_start_time(timestamp.into());
        self.log_static(EntityPath::recording_properties(), &update)
    }

    // NOTE: For bw and fw compatibility reasons, we need our logging APIs to be fallible, even
    // though they really aren't at the moment.
    #[allow(clippy::unnecessary_wraps)]
    fn log_serialized_batches_impl(
        &self,
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        static_: bool,
        comp_batches: impl IntoIterator<Item = re_types::SerializedComponentBatch>,
    ) -> RecordingStreamResult<()> {
        if !self.is_enabled() {
            return Ok(()); // silently drop the message
        }

        let entity_path = entity_path.into();

        let comp_batches: Vec<_> = comp_batches
            .into_iter()
            .map(|comp_batch| (comp_batch.descriptor, comp_batch.array))
            .collect();
        let components: IntMap<_, _> = comp_batches.into_iter().collect();

        // NOTE: The timepoint is irrelevant, the `RecordingStream` will overwrite it using its
        // internal clock.
        let timepoint = TimePoint::default();

        if !components.is_empty() {
            let row = PendingRow {
                row_id,
                timepoint,
                components,
            };
            self.record_row(entity_path, row, !static_);
        }

        Ok(())
    }

    /// Logs the file at the given `path` using all [`re_data_loader::DataLoader`]s available.
    ///
    /// A single `path` might be handled by more than one loader.
    ///
    /// This method blocks until either at least one [`re_data_loader::DataLoader`] starts
    /// streaming data in or all of them fail.
    ///
    /// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
    #[cfg(feature = "data_loaders")]
    pub fn log_file_from_path(
        &self,
        filepath: impl AsRef<std::path::Path>,
        entity_path_prefix: Option<EntityPath>,
        static_: bool,
    ) -> RecordingStreamResult<()> {
        self.log_file(filepath, None, entity_path_prefix, static_, true)
    }

    /// Logs the given `contents` using all [`re_data_loader::DataLoader`]s available.
    ///
    /// A single `path` might be handled by more than one loader.
    ///
    /// This method blocks until either at least one [`re_data_loader::DataLoader`] starts
    /// streaming data in or all of them fail.
    ///
    /// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
    #[cfg(feature = "data_loaders")]
    pub fn log_file_from_contents(
        &self,
        filepath: impl AsRef<std::path::Path>,
        contents: std::borrow::Cow<'_, [u8]>,
        entity_path_prefix: Option<EntityPath>,
        static_: bool,
    ) -> RecordingStreamResult<()> {
        self.log_file(filepath, Some(contents), entity_path_prefix, static_, true)
    }

    /// If `prefer_current_recording` is set (which is always the case for now), the dataloader settings
    /// will be configured as if the current SDK recording is the currently opened recording.
    /// Most dataloaders prefer logging to the currently opened recording if one is set.
    #[cfg(feature = "data_loaders")]
    fn log_file(
        &self,
        filepath: impl AsRef<std::path::Path>,
        contents: Option<std::borrow::Cow<'_, [u8]>>,
        entity_path_prefix: Option<EntityPath>,
        static_: bool,
        prefer_current_recording: bool,
    ) -> RecordingStreamResult<()> {
        let Some(store_info) = self.store_info().clone() else {
            re_log::warn!("Ignored call to log_file() because RecordingStream has not been properly initialized");
            return Ok(());
        };

        let filepath = filepath.as_ref();
        let has_contents = contents.is_some();

        let (tx, rx) = re_smart_channel::smart_channel(
            re_smart_channel::SmartMessageSource::Sdk,
            re_smart_channel::SmartChannelSource::File(filepath.into()),
        );

        let mut settings = crate::DataLoaderSettings {
            application_id: Some(store_info.application_id.clone()),
            opened_application_id: None,
            store_id: store_info.store_id.clone(),
            opened_store_id: None,
            force_store_info: false,
            entity_path_prefix,
            timepoint: (!static_).then(|| {
                self.with(|inner| {
                    // Get the current time on all timelines, for the current recording, on the current
                    // thread…
                    let mut now = self.now();

                    // …and then also inject the current recording tick into it.
                    let tick = inner
                        .tick
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    now.insert_cell(TimelineName::log_tick(), TimeCell::from_sequence(tick));

                    now
                })
                .unwrap_or_default()
            }),
        };

        if prefer_current_recording {
            settings.opened_application_id = Some(store_info.application_id.clone());
            settings.opened_store_id = Some(store_info.store_id);
        }

        if let Some(contents) = contents {
            re_data_loader::load_from_file_contents(
                &settings,
                re_log_types::FileSource::Sdk,
                filepath,
                contents,
                &tx,
            )?;
        } else {
            re_data_loader::load_from_path(
                &settings,
                re_log_types::FileSource::Sdk,
                filepath,
                &tx,
            )?;
        }
        drop(tx);

        // We can safely ignore the error on `recv()` as we're in complete control of both ends of
        // the channel.
        let thread_name = if has_contents {
            format!("log_file_from_contents({filepath:?})")
        } else {
            format!("log_file_from_path({filepath:?})")
        };
        let handle = std::thread::Builder::new()
            .name(thread_name.clone())
            .spawn({
                let this = self.clone_weak();
                move || {
                    while let Some(msg) = rx.recv().ok().and_then(|msg| msg.into_data()) {
                        this.record_msg(msg);
                    }
                }
            })
            .map_err(|err| RecordingStreamError::SpawnThread {
                name: thread_name,
                err,
            })?;

        self.with(|inner| inner.dataloader_handles.lock().push(handle));

        Ok(())
    }
}

#[allow(clippy::needless_pass_by_value)]
fn forwarding_thread(
    info: StoreInfo,
    mut sink: Box<dyn LogSink>,
    cmds_rx: Receiver<Command>,
    chunks: Receiver<Chunk>,
    on_release: Option<ArrowRecordBatchReleaseCallback>,
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
                            row_id: *RowId::new(),
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
            Command::PopPendingChunks => {
                // Wake up and skip the current iteration so that we can drain all pending chunks
                // before handling the next command.
            }
            Command::Shutdown => return false,
        }

        true
    }

    use crossbeam::select;
    loop {
        // NOTE: Always pop chunks first, this is what makes `Command::PopPendingChunks` possible,
        // which in turns makes `RecordingStream::flush_blocking` well defined.
        while let Ok(chunk) = chunks.try_recv() {
            let mut msg = match chunk.to_arrow_msg() {
                Ok(chunk) => chunk,
                Err(err) => {
                    re_log::error!(%err, "couldn't serialize chunk; data dropped (this is a bug in Rerun!)");
                    continue;
                }
            };
            msg.on_release = on_release.clone();
            sink.send(LogMsg::ArrowMsg(info.store_id.clone(), msg));
        }

        select! {
            recv(chunks) -> res => {
                let Ok(chunk) = res else {
                    // The batcher is gone, which can only happen if the `RecordingStream` itself
                    // has been dropped.
                    re_log::trace!("Shutting down forwarding_thread: batcher is gone");
                    break;
                };

                let msg = match chunk.to_arrow_msg() {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        re_log::error!(%err, "couldn't serialize chunk; data dropped (this is a bug in Rerun!)");
                        continue;
                    }
                };

                sink.send(LogMsg::ArrowMsg(info.store_id.clone(), msg));
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
        self.with(|_| true).unwrap_or(false)
    }

    /// The [`StoreInfo`] associated with this `RecordingStream`.
    #[inline]
    pub fn store_info(&self) -> Option<StoreInfo> {
        self.with(|inner| inner.info.clone())
    }

    /// Determine whether a fork has happened since creating this `RecordingStream`. In general, this means our
    /// batcher/sink threads are gone and all data logged since the fork has been dropped.
    ///
    /// It is essential that [`crate::cleanup_if_forked_child`] be called after forking the process. SDK-implementations
    /// should do this during their initialization phase.
    #[inline]
    pub fn is_forked_child(&self) -> bool {
        self.with(|inner| inner.is_forked_child()).unwrap_or(false)
    }
}

impl RecordingStream {
    /// Records an arbitrary [`LogMsg`].
    #[inline]
    pub fn record_msg(&self, msg: LogMsg) {
        let f = move |inner: &RecordingStreamInner| {
            // NOTE: Internal channels can never be closed outside of the `Drop` impl, this send cannot
            // fail.
            inner.cmds_tx.send(Command::RecordMsg(msg)).ok();
            inner
                .tick
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to record_msg() ignored");
        }
    }

    /// Records a single [`PendingRow`].
    ///
    /// If `inject_time` is set to `true`, the row's timestamp data will be overridden using the
    /// [`RecordingStream`]'s internal clock.
    ///
    /// Internally, incoming [`PendingRow`]s are automatically coalesced into larger [`Chunk`]s to
    /// optimize for transport.
    #[inline]
    pub fn record_row(&self, entity_path: EntityPath, mut row: PendingRow, inject_time: bool) {
        let f = move |inner: &RecordingStreamInner| {
            // NOTE: We're incrementing the current tick still.
            let tick = inner
                .tick
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if inject_time {
                // Get the current time on all timelines, for the current recording, on the current
                // thread…
                let mut now = self.now();
                // …and then also inject the current recording tick into it.
                now.insert_cell(TimelineName::log_tick(), TimeCell::from_sequence(tick));

                // Inject all these times into the row, overriding conflicting times, if any.
                for (timeline, cell) in now {
                    row.timepoint.insert_cell(timeline, cell);
                }
            }

            inner.batcher.push_row(entity_path, row);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to record_row() ignored");
        }
    }

    /// Logs a single [`Chunk`].
    ///
    /// Will inject `log_tick` and `log_time` timeline columns into the chunk.
    /// If you don't want to inject these, use [`Self::send_chunk`] instead.
    #[inline]
    pub fn log_chunk(&self, mut chunk: Chunk) {
        let f = move |inner: &RecordingStreamInner| {
            // TODO(cmc): Repeating these values is pretty wasteful. Would be nice to have a way of
            // indicating these are fixed across the whole chunk.
            // Inject the log time
            {
                let time_timeline = Timeline::log_time();
                let time =
                    TimeInt::new_temporal(re_log_types::Timestamp::now().nanos_since_epoch());

                let repeated_time = std::iter::repeat(time.as_i64())
                    .take(chunk.num_rows())
                    .collect();

                let time_column = TimeColumn::new(Some(true), time_timeline, repeated_time);

                if let Err(err) = chunk.add_timeline(time_column) {
                    re_log::error!(
                        "Couldn't inject '{}' timeline into chunk (this is a bug in Rerun!): {}",
                        time_timeline.name(),
                        err
                    );
                    return;
                }
            }
            // Inject the log tick
            {
                let tick_timeline = Timeline::log_tick();

                let tick = inner
                    .tick
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                let repeated_tick = std::iter::repeat(tick).take(chunk.num_rows()).collect();

                let tick_chunk = TimeColumn::new(Some(true), tick_timeline, repeated_tick);

                if let Err(err) = chunk.add_timeline(tick_chunk) {
                    re_log::error!(
                        "Couldn't inject '{}' timeline into chunk (this is a bug in Rerun!): {}",
                        tick_timeline.name(),
                        err
                    );
                    return;
                }
            }

            inner.batcher.push_chunk(chunk);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to log_chunk() ignored");
        }
    }

    /// Records a single [`Chunk`].
    ///
    /// This will _not_ inject `log_tick` and `log_time` timeline columns into the chunk,
    /// for that use [`Self::log_chunk`].
    #[inline]
    pub fn send_chunk(&self, chunk: Chunk) {
        let f = move |inner: &RecordingStreamInner| {
            inner.batcher.push_chunk(chunk);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to send_chunk() ignored");
        }
    }

    /// Swaps the underlying sink for a new one.
    ///
    /// This guarantees that:
    /// 1. all pending rows and chunks are batched, collected and sent down the current sink,
    /// 2. the current sink is flushed if it has pending data in its buffers,
    /// 3. the current sink's backlog, if there's any, is forwarded to the new sink.
    ///
    /// When this function returns, the calling thread is guaranteed that all future record calls
    /// will end up in the new sink.
    ///
    /// ## Data loss
    ///
    /// If the current sink is in a broken state (e.g. a gRPC sink with a broken connection that
    /// cannot be repaired), all pending data in its buffers will be dropped.
    pub fn set_sink(&self, sink: Box<dyn LogSink>) {
        if self.is_forked_child() {
            re_log::error_once!("Fork detected during set_sink. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK.");
            return;
        }

        let f = move |inner: &RecordingStreamInner| {
            // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
            // are safe.

            // 1. Flush the batcher down the chunk channel
            inner.batcher.flush_blocking();

            // 2. Receive pending chunks from the batcher's channel
            inner.cmds_tx.send(Command::PopPendingChunks).ok();

            // 3. Swap the sink, which will internally make sure to re-ingest the backlog if needed
            inner.cmds_tx.send(Command::SwapSink(sink)).ok();

            // 4. Before we give control back to the caller, we need to make sure that the swap has
            //    taken place: we don't want the user to send data to the old sink!
            re_log::trace!("Waiting for sink swap to complete…");
            let (cmd, oneshot) = Command::flush();
            inner.cmds_tx.send(cmd).ok();
            oneshot.recv().ok();
            re_log::trace!("Sink swap completed.");
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to set_sink() ignored");
        }
    }

    /// Initiates a flush of the pipeline and returns immediately.
    ///
    /// This does **not** wait for the flush to propagate (see [`Self::flush_blocking`]).
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    pub fn flush_async(&self) {
        if self.is_forked_child() {
            re_log::error_once!("Fork detected during flush_async. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK.");
            return;
        }

        let f = move |inner: &RecordingStreamInner| {
            // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
            // are safe.

            // 1. Synchronously flush the batcher down the chunk channel
            //
            // NOTE: This _has_ to be done synchronously as we need to be guaranteed that all chunks
            // are ready to be drained by the time this call returns.
            // It cannot block indefinitely and is fairly fast as it only requires compute (no I/O).
            inner.batcher.flush_blocking();

            // 2. Drain all pending chunks from the batcher's channel _before_ any other future command
            inner.cmds_tx.send(Command::PopPendingChunks).ok();

            // 3. Asynchronously flush everything down the sink
            let (cmd, _) = Command::flush();
            inner.cmds_tx.send(cmd).ok();
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to flush_async() ignored");
        }
    }

    /// Initiates a flush the batching pipeline and waits for it to propagate.
    ///
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    pub fn flush_blocking(&self) {
        if self.is_forked_child() {
            re_log::error_once!("Fork detected during flush. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK.");
            return;
        }

        let f = move |inner: &RecordingStreamInner| {
            // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
            // are safe.

            // 1. Flush the batcher down the chunk channel
            inner.batcher.flush_blocking();

            // 2. Drain all pending chunks from the batcher's channel _before_ any other future command
            inner.cmds_tx.send(Command::PopPendingChunks).ok();

            // 3. Wait for all chunks to have been forwarded down the sink
            let (cmd, oneshot) = Command::flush();
            inner.cmds_tx.send(cmd).ok();
            oneshot.recv().ok();
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to flush_blocking() ignored");
        }
    }
}

impl RecordingStream {
    /// Swaps the underlying sink for a [`crate::log_sink::GrpcSink`] sink pre-configured to use
    /// the specified address.
    ///
    /// See also [`Self::connect_grpc_opts`] if you wish to configure the connection.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn connect_grpc(&self) -> RecordingStreamResult<()> {
        self.connect_grpc_opts(
            format!(
                "rerun+http://127.0.0.1:{}/proxy",
                re_grpc_server::DEFAULT_SERVER_PORT
            ),
            crate::default_flush_timeout(),
        )
    }

    /// Swaps the underlying sink for a [`crate::log_sink::GrpcSink`] sink pre-configured to use
    /// the specified address.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    ///
    /// `flush_timeout` is the minimum time the [`GrpcSink`][`crate::log_sink::GrpcSink`] will
    /// wait during a flush before potentially dropping data. Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    pub fn connect_grpc_opts(
        &self,
        url: impl Into<String>,
        flush_timeout: Option<Duration>,
    ) -> RecordingStreamResult<()> {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new GrpcSink since {ENV_FORCE_SAVE} is set");
            return Ok(());
        }

        let url: String = url.into();
        let re_uri::RedapUri::Proxy(endpoint) = re_uri::RedapUri::try_from(url.as_str())? else {
            return Err(RecordingStreamError::NotAProxyEndpoint);
        };

        let sink = crate::log_sink::GrpcSink::new(endpoint, flush_timeout);

        self.set_sink(Box::new(sink));
        Ok(())
    }

    /// Spawns a new Rerun Viewer process from an executable available in PATH, then swaps the
    /// underlying sink for a [`crate::log_sink::GrpcSink`] sink pre-configured to send data to that
    /// new process.
    ///
    /// If a Rerun Viewer is already listening on this port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// See also [`Self::spawn_opts`] if you wish to configure the behavior of thew Rerun process
    /// as well as the underlying connection.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn spawn(&self) -> RecordingStreamResult<()> {
        self.spawn_opts(&Default::default(), crate::default_flush_timeout())
    }

    /// Spawns a new Rerun Viewer process from an executable available in PATH, then swaps the
    /// underlying sink for a [`crate::log_sink::GrpcSink`] sink pre-configured to send data to that
    /// new process.
    ///
    /// If a Rerun Viewer is already listening on this port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// The behavior of the spawned Viewer can be configured via `opts`.
    /// If you're fine with the default behavior, refer to the simpler [`Self::spawn`].
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    ///
    /// `flush_timeout` is the minimum time the [`GrpcSink`][`crate::log_sink::GrpcSink`] will
    /// wait during a flush before potentially dropping data. Note: Passing `None` here can cause a
    /// call to `flush` to block indefinitely if a connection cannot be established.
    pub fn spawn_opts(
        &self,
        opts: &crate::SpawnOptions,
        flush_timeout: Option<Duration>,
    ) -> RecordingStreamResult<()> {
        if !self.is_enabled() {
            re_log::debug!("Rerun disabled - call to spawn() ignored");
            return Ok(());
        }
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new GrpcSink since {ENV_FORCE_SAVE} is set");
            return Ok(());
        }

        crate::spawn(opts)?;

        self.connect_grpc_opts(
            format!("rerun+http://{}/proxy", opts.connect_addr()),
            flush_timeout,
        )?;

        Ok(())
    }

    /// Swaps the underlying sink for a [`crate::sink::MemorySink`] sink and returns the associated
    /// [`MemorySinkStorage`].
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn memory(&self) -> MemorySinkStorage {
        let sink = crate::sink::MemorySink::new(self.clone());
        let storage = sink.buffer();
        self.set_sink(Box::new(sink));
        storage
    }

    /// Swaps the underlying sink for a [`crate::sink::BinaryStreamSink`] sink and returns the associated
    /// [`BinaryStreamStorage`].
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn binary_stream(&self) -> Result<BinaryStreamStorage, crate::sink::BinaryStreamSinkError> {
        let (sink, storage) = crate::sink::BinaryStreamSink::new(self.clone())?;
        self.set_sink(Box::new(sink));
        Ok(storage)
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
        self.save_opts(path)
    }

    /// Swaps the underlying sink for a [`crate::sink::FileSink`] at the specified `path`.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    ///
    /// If a blueprint was provided, it will be stored first in the file.
    /// Blueprints are currently an experimental part of the Rust SDK.
    pub fn save_opts(
        &self,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<(), crate::sink::FileSinkError> {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new file since {ENV_FORCE_SAVE} is set");
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
        self.stdout_opts()
    }

    /// Swaps the underlying sink for a [`crate::sink::FileSink`] pointed at stdout.
    ///
    /// If there isn't any listener at the other end of the pipe, the [`RecordingStream`] will
    /// default back to `buffered` mode, in order not to break the user's terminal.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    ///
    /// If a blueprint was provided, it will be stored first in the file.
    /// Blueprints are currently an experimental part of the Rust SDK.
    pub fn stdout_opts(&self) -> Result<(), crate::sink::FileSinkError> {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new file since {ENV_FORCE_SAVE} is set");
            return Ok(());
        }

        if std::io::stdout().is_terminal() {
            re_log::debug!("Ignored call to stdout() because stdout is a terminal");
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
        let f = move |inner: &RecordingStreamInner| {
            // When disconnecting, we need to make sure that pending top-level `DataLoader` threads that
            // were started from the SDK run to completion.
            inner.wait_for_dataloaders();
            self.set_sink(Box::new(crate::sink::BufferedSink::new()));
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to disconnect() ignored");
        }
    }

    /// Send a blueprint through this recording stream
    pub fn send_blueprint(
        &self,
        blueprint: Vec<LogMsg>,
        activation_cmd: BlueprintActivationCommand,
    ) {
        let mut blueprint_id = None;
        for msg in blueprint {
            if blueprint_id.is_none() {
                blueprint_id = Some(msg.store_id().clone());
            }
            self.record_msg(msg);
        }

        if let Some(blueprint_id) = blueprint_id {
            if blueprint_id == activation_cmd.blueprint_id {
                // Let the viewer know that the blueprint has been fully received,
                // and that it can now be activated.
                // We don't want to activate half-loaded blueprints, because that can be confusing,
                // and can also lead to problems with view heuristics.
                self.record_msg(activation_cmd.into());
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

impl fmt::Debug for RecordingStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let with = |inner: &RecordingStreamInner| {
            let RecordingStreamInner {
                // This pattern match prevents _accidentally_ omitting data from the debug output
                // when new fields are added.
                info,
                properties,
                tick,
                cmds_tx: _,
                batcher: _,
                batcher_to_sink_handle: _,
                dataloader_handles,
                pid_at_creation,
            } = inner;

            f.debug_struct("RecordingStream")
                .field("info", &info)
                .field("properties", &properties)
                .field("tick", &tick)
                .field("pending_dataloaders", &dataloader_handles.lock().len())
                .field("pid_at_creation", &pid_at_creation)
                .finish_non_exhaustive()
        };

        match self.with(with) {
            Some(res) => res,
            None => write!(f, "RecordingStream {{ disabled }}"),
        }
    }
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

    fn set_thread_time(rid: &StoreId, timeline: TimelineName, cell: TimeCell) {
        Self::with(|ti| ti.set_time(rid, timeline, cell));
    }

    fn unset_thread_time(rid: &StoreId, timeline: &TimelineName) {
        Self::with(|ti| ti.unset_time(rid, timeline));
    }

    fn reset_thread_time(rid: &StoreId) {
        Self::with(|ti| ti.reset_time(rid));
    }

    /// Get access to the thread-local [`ThreadInfo`].
    fn with<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        use std::cell::RefCell;
        thread_local! {
            static THREAD_INFO: RefCell<Option<ThreadInfo>> = const { RefCell::new(None) };
        }

        THREAD_INFO.with(|thread_info| {
            let mut thread_info = thread_info.borrow_mut();
            let thread_info = thread_info.get_or_insert_with(Self::default);
            f(thread_info)
        })
    }

    fn now(&self, rid: &StoreId) -> TimePoint {
        let mut timepoint = self.timepoints.get(rid).cloned().unwrap_or_default();
        timepoint.insert_cell(TimelineName::log_time(), TimeCell::timestamp_now());
        timepoint
    }

    fn set_time(&mut self, rid: &StoreId, timeline: TimelineName, cell: TimeCell) {
        self.timepoints
            .entry(rid.clone())
            .or_default()
            .insert_cell(timeline, cell);
    }

    fn unset_time(&mut self, rid: &StoreId, timeline: &TimelineName) {
        if let Some(timepoint) = self.timepoints.get_mut(rid) {
            timepoint.remove(timeline);
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
        let f = move |inner: &RecordingStreamInner| ThreadInfo::thread_now(&inner.info.store_id);
        if let Some(res) = self.with(f) {
            res
        } else {
            re_log::warn_once!("Recording disabled - call to now() ignored");
            TimePoint::default()
        }
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_duration_seconds`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_timepoint(&self, timepoint: impl Into<TimePoint>) {
        let f = move |inner: &RecordingStreamInner| {
            let timepoint = timepoint.into();
            for (timeline, time) in timepoint {
                ThreadInfo::set_thread_time(&inner.info.store_id, timeline, time);
            }
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to set_timepoint() ignored");
        }
    }

    /// Set the current value of one of the timelines.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// Example:
    /// ```no_run
    /// # mod rerun { pub use re_sdk::*; }
    /// # let rec: rerun::RecordingStream = unimplemented!();
    /// rec.set_time("frame_nr", rerun::TimeCell::from_sequence(42));
    /// rec.set_time("duration", std::time::Duration::from_millis(123));
    /// rec.set_time("capture_time", std::time::SystemTime::now());
    /// ```
    ///
    /// See also:
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_duration_seconds`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_time(&self, timeline: impl Into<TimelineName>, value: impl TryInto<TimeCell>) {
        let f = move |inner: &RecordingStreamInner| {
            let timeline = timeline.into();
            if let Ok(value) = value.try_into() {
                ThreadInfo::set_thread_time(&inner.info.store_id, timeline, value);
            } else {
                re_log::warn_once!(
                    "set_time({timeline}): Failed to convert the given value to an TimeCell"
                );
            }
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to set_time() ignored");
        }
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Short for `set_time(timeline, rerun::TimeCell::from_sequence(sequence))`.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
    ///
    /// For example: `rec.set_time_sequence("frame_nr", frame_nr)`.
    /// You can remove a timeline again using `rec.disable_timeline("frame_nr")`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time`]
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_duration_seconds`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    #[inline]
    pub fn set_time_sequence(&self, timeline: impl Into<TimelineName>, sequence: impl Into<i64>) {
        self.set_time(timeline, TimeCell::from_sequence(sequence.into()));
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Short for `set_time(timeline, std::time::Duration::from_secs_f64(secs))`..
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
    ///
    /// For example: `rec.set_duration_seconds("time_since_start", time_offset)`.
    /// You can remove a timeline again using `rec.disable_timeline("time_since_start")`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time`]
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_timestamp_seconds_since_epoch`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    #[inline]
    pub fn set_duration_seconds(&self, timeline: impl Into<TimelineName>, secs: impl Into<f64>) {
        self.set_time(timeline, std::time::Duration::from_secs_f64(secs.into()));
    }

    /// Set a timestamp as seconds since Unix epoch (1970-01-01 00:00:00 UTC).
    ///
    /// Short for `self.set_time(timeline, rerun::TimeCell::from_timestamp_seconds_since_epoch(secs))`.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
    ///
    /// For example: `rec.set_duration_seconds("time_since_start", time_offset)`.
    /// You can remove a timeline again using `rec.disable_timeline("time_since_start")`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time`]
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_duration_seconds`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    #[inline]
    pub fn set_timestamp_seconds_since_epoch(
        &self,
        timeline: impl Into<TimelineName>,
        secs: impl Into<f64>,
    ) {
        self.set_time(
            timeline,
            TimeCell::from_timestamp_seconds_since_epoch(secs.into()),
        );
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
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
    #[deprecated(
        since = "0.23.0",
        note = "Use either `set_duration_seconds` or `set_timestamp_seconds_since_epoch` instead"
    )]
    #[inline]
    pub fn set_time_seconds(&self, timeline: impl Into<TimelineName>, seconds: impl Into<f64>) {
        self.set_duration_seconds(timeline, seconds);
    }

    /// Set the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
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
    #[deprecated(
        since = "0.23.0",
        note = "Use `set_time` with either `rerun::TimeCell::from_duration_nanos` or `rerun::TimeCell::from_timestamp_nanos_since_epoch`, or with `std::time::Duration` or `std::time::SystemTime`."
    )]
    #[inline]
    pub fn set_time_nanos(
        &self,
        timeline: impl Into<TimelineName>,
        nanos_since_epoch: impl Into<i64>,
    ) {
        self.set_time(
            timeline,
            TimeCell::from_timestamp_nanos_since_epoch(nanos_since_epoch.into()),
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
        let f = move |inner: &RecordingStreamInner| {
            let timeline = timeline.into();
            ThreadInfo::unset_thread_time(&inner.info.store_id, &timeline);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to disable_timeline() ignored");
        }
    }

    /// Clears out the current time of the recording, for the current calling thread.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
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
        let f = move |inner: &RecordingStreamInner| {
            ThreadInfo::reset_thread_time(&inner.info.store_id);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to reset_time() ignored");
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use re_log_types::example_components::MyLabel;

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
            .batcher_config(ChunkBatcherConfig::NEVER)
            .buffered()
            .unwrap();

        let store_info = rec.store_info().unwrap();

        let rows = example_rows(false);
        for row in rows.clone() {
            rec.record_row("a".into(), row, false);
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
                assert!(msg.row_id != *RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            _ => panic!("expected SetStoreInfo"),
        }

        // Second message should be a set_store_info resulting from the later sink swap from
        // buffered mode into in-memory mode.
        // This arrives _before_ the data itself since we're using manual flushing.
        match msgs.pop().unwrap() {
            LogMsg::SetStoreInfo(msg) => {
                assert!(msg.row_id != *RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            _ => panic!("expected SetStoreInfo"),
        }

        // The following flushes were sent as a result of the implicit flush when swapping the
        // underlying sink from buffered to in-memory.

        // Chunk that contains the `RecordProperties`.
        match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(store_info.store_id, rid);

                let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                chunk.sanity_check().unwrap();
            }
            _ => panic!("expected ArrowMsg"),
        }

        // Another chunk that contains `RecordProperties`.
        match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(store_info.store_id, rid);

                let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                chunk.sanity_check().unwrap();
            }
            _ => panic!("expected ArrowMsg"),
        }

        // Final message is the batched chunk itself.
        match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(store_info.store_id, rid);

                let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                chunk.sanity_check().unwrap();
            }
            _ => panic!("expected ArrowMsg"),
        }

        // That's all.
        assert!(msgs.pop().is_none());
    }

    #[test]
    fn always_flush() {
        let rec = RecordingStreamBuilder::new("rerun_example_always_flush")
            .enabled(true)
            .batcher_config(ChunkBatcherConfig::ALWAYS)
            .buffered()
            .unwrap();

        let store_info = rec.store_info().unwrap();

        let rows = example_rows(false);
        for row in rows.clone() {
            rec.record_row("a".into(), row, false);
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
                assert!(msg.row_id != *RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            _ => panic!("expected SetStoreInfo"),
        }

        // Second message should be a set_store_info resulting from the later sink swap from
        // buffered mode into in-memory mode.
        // This arrives _before_ the data itself since we're using manual flushing.
        match msgs.pop().unwrap() {
            LogMsg::SetStoreInfo(msg) => {
                assert!(msg.row_id != *RowId::ZERO);
                similar_asserts::assert_eq!(store_info, msg.info);
            }
            _ => panic!("expected SetStoreInfo"),
        }

        let mut assert_next_row = || match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(store_info.store_id, rid);

                let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                chunk.sanity_check().unwrap();
            }
            _ => panic!("expected ArrowMsg"),
        };

        // 3rd, 4th, 5th, 6th, and 7th messages are all the single-row batched chunks themselves,
        // which were sent as a result of the implicit flush when swapping the underlying sink
        // from buffered to in-memory. Note that these messages contain the 2 recording property
        // chunks.
        assert_next_row();
        assert_next_row();
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
            .batcher_config(ChunkBatcherConfig::NEVER)
            .memory()
            .unwrap();

        let store_info = rec.store_info().unwrap();

        let rows = example_rows(false);
        for row in rows.clone() {
            rec.record_row("a".into(), row, false);
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
                    assert!(msg.row_id != *RowId::ZERO);
                    similar_asserts::assert_eq!(store_info, msg.info);
                }
                _ => panic!("expected SetStoreInfo"),
            }

            // For reasons, MemorySink ends up with 2 StoreInfos.
            // TODO(jleibs): Avoid a redundant StoreInfo message.
            match msgs.pop().unwrap() {
                LogMsg::SetStoreInfo(msg) => {
                    assert!(msg.row_id != *RowId::ZERO);
                    similar_asserts::assert_eq!(store_info, msg.info);
                }
                _ => panic!("expected SetStoreInfo"),
            }

            // MemorySinkStorage transparently handles flushing during `take()`!

            // The batch that contains the `RecordingProperties`.
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(store_info.store_id, rid);

                    let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                    chunk.sanity_check().unwrap();
                }
                _ => panic!("expected ArrowMsg"),
            }

            // For the same reasons as above, another chunk that contains the `RecordingProperties`.
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(store_info.store_id, rid);

                    let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                    chunk.sanity_check().unwrap();
                }
                _ => panic!("expected ArrowMsg"),
            }

            // The batched chunk itself, which was sent as a result of the explicit flush above.
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(store_info.store_id, rid);

                    let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                    chunk.sanity_check().unwrap();
                }
                _ => panic!("expected ArrowMsg"),
            }

            // That's all.
            assert!(msgs.pop().is_none());
        }
    }

    #[test]
    fn disabled() {
        let (rec, storage) = RecordingStreamBuilder::new("rerun_example_disabled")
            .enabled(false)
            .batcher_config(ChunkBatcherConfig::ALWAYS)
            .memory()
            .unwrap();

        let rows = example_rows(false);
        for row in rows.clone() {
            rec.record_row("a".into(), row, false);
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

    fn example_rows(static_: bool) -> Vec<PendingRow> {
        use re_log_types::example_components::{MyColor, MyLabel, MyPoint};
        use re_types::{Component as _, Loggable};

        let mut tick = 0i64;
        let mut timepoint = |frame_nr: i64| {
            let mut tp = TimePoint::default();
            if !static_ {
                tp.insert(Timeline::log_time(), re_log_types::Timestamp::now());
                tp.insert(Timeline::log_tick(), tick);
                tp.insert(Timeline::new_sequence("frame_nr"), frame_nr);
            }
            tick += 1;
            tp
        };

        let row0 = {
            PendingRow {
                row_id: RowId::new(),
                timepoint: timepoint(1),
                components: [
                    (
                        MyPoint::descriptor(),
                        <MyPoint as Loggable>::to_arrow([
                            MyPoint::new(10.0, 10.0),
                            MyPoint::new(20.0, 20.0),
                        ])
                        .unwrap(),
                    ), //
                    (
                        MyColor::descriptor(),
                        <MyColor as Loggable>::to_arrow([MyColor(0x8080_80FF)]).unwrap(),
                    ), //
                    (
                        MyLabel::descriptor(),
                        <MyLabel as Loggable>::to_arrow([] as [MyLabel; 0]).unwrap(),
                    ), //
                ]
                .into_iter()
                .collect(),
            }
        };

        let row1 = {
            PendingRow {
                row_id: RowId::new(),
                timepoint: timepoint(1),
                components: [
                    (
                        MyPoint::descriptor(),
                        <MyPoint as Loggable>::to_arrow([] as [MyPoint; 0]).unwrap(),
                    ), //
                    (
                        MyColor::descriptor(),
                        <MyColor as Loggable>::to_arrow([] as [MyColor; 0]).unwrap(),
                    ), //
                    (
                        MyLabel::descriptor(),
                        <MyLabel as Loggable>::to_arrow([] as [MyLabel; 0]).unwrap(),
                    ), //
                ]
                .into_iter()
                .collect(),
            }
        };

        let row2 = {
            PendingRow {
                row_id: RowId::new(),
                timepoint: timepoint(1),
                components: [
                    (
                        MyPoint::descriptor(),
                        <MyPoint as Loggable>::to_arrow([] as [MyPoint; 0]).unwrap(),
                    ), //
                    (
                        MyColor::descriptor(),
                        <MyColor as Loggable>::to_arrow([MyColor(0xFFFF_FFFF)]).unwrap(),
                    ), //
                    (
                        MyLabel::descriptor(),
                        <MyLabel as Loggable>::to_arrow([MyLabel("hey".into())]).unwrap(),
                    ), //
                ]
                .into_iter()
                .collect(),
            }
        };

        vec![row0, row1, row2]
    }

    // See <https://github.com/rerun-io/rerun/pull/8587> for context.
    #[test]
    fn allows_componentbatch_unsized() {
        let labels = [
            MyLabel("a".into()),
            MyLabel("b".into()),
            MyLabel("c".into()),
        ];

        let (rec, _mem) = RecordingStreamBuilder::new("rerun_example_test_componentbatch_unsized")
            .default_enabled(false)
            .enabled(false)
            .memory()
            .unwrap();

        // This call used to *not* compile due to a lack of `?Sized` bounds.
        use re_types::ComponentBatch as _;
        rec.log("labels", &labels.try_serialized().unwrap())
            .unwrap();
    }
}
