use std::fmt;
use std::io::IsTerminal as _;
use std::sync::atomic::AtomicI64;
use std::sync::{Arc, Weak};
use std::time::Duration;

use ahash::HashMap;
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use itertools::Either;
use nohash_hasher::IntMap;
use parking_lot::Mutex;
use re_chunk::{
    BatcherFlushError, BatcherHooks, Chunk, ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError,
    ChunkComponents, ChunkError, ChunkId, PendingRow, RowId, TimeColumn,
};
use re_log_types::{
    ApplicationId, ArrowRecordBatchReleaseCallback, BlueprintActivationCommand, EntityPath, LogMsg,
    RecordingId, StoreId, StoreInfo, StoreKind, StoreSource, TimeCell, TimeInt, TimePoint,
    Timeline, TimelineName,
};
use re_quota_channel::send_crossbeam;
use re_sdk_types::archetypes::RecordingInfo;
use re_sdk_types::components::Timestamp;
use re_sdk_types::{AsComponents, SerializationError, SerializedComponentColumn};

use crate::binary_stream_sink::BinaryStreamStorage;
use crate::sink::{LogSink, MemorySinkStorage, SinkFlushError};

// ---

/// Private environment variable meant for tests.
///
/// When set, all recording streams will write to disk at the path indicated by the env-var rather
/// than doing what they were asked to do - `connect_grpc()`, `buffered()`, even `save()` will re-use the same sink.
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

    /// Invalid bind IP.
    #[error(transparent)]
    InvalidAddress(#[from] std::net::AddrParseError),
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
/// Automatically sends a [`Chunk`] with the default [`RecordingInfo`] to
/// the sink, unless an explicit `recording_id` is set via [`RecordingStreamBuilder::recording_id`].
#[derive(Debug)]
pub struct RecordingStreamBuilder {
    application_id: ApplicationId,
    store_kind: StoreKind,
    recording_id: Option<RecordingId>,
    store_source: Option<StoreSource>,

    default_enabled: bool,
    enabled: Option<bool>,

    batcher_hooks: BatcherHooks,
    batcher_config: Option<ChunkBatcherConfig>,

    // Optional user-defined recording properties.
    should_send_properties: bool,
    recording_info: RecordingInfo,

    /// Optional blueprint with activation settings.
    blueprint: Option<crate::blueprint::BlueprintOpts>,
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
            recording_id: None,
            store_source: None,

            default_enabled: true,
            enabled: None,

            batcher_config: None,
            batcher_hooks: BatcherHooks::NONE,

            should_send_properties: true,
            recording_info: RecordingInfo::new()
                .with_start_time(re_sdk_types::components::Timestamp::now()),

            blueprint: None,
        }
    }

    /// Create a new [`RecordingStreamBuilder`] with the given [`StoreId`].
    //
    // NOTE: track_caller so that we can see if we are being called from an official example.
    #[track_caller]
    pub fn from_store_id(store_id: &StoreId) -> Self {
        Self {
            application_id: store_id.application_id().clone(),
            store_kind: store_id.kind(),
            recording_id: Some(store_id.recording_id().clone()),
            store_source: None,

            default_enabled: true,
            enabled: None,

            batcher_config: None,
            batcher_hooks: BatcherHooks::NONE,

            should_send_properties: true,
            recording_info: RecordingInfo::new()
                .with_start_time(re_sdk_types::components::Timestamp::now()),

            blueprint: None,
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
    pub fn recording_id(mut self, recording_id: impl Into<RecordingId>) -> Self {
        self.recording_id = Some(recording_id.into());
        self.send_properties(false)
    }

    /// Sets an optional name for the recording.
    #[inline]
    pub fn recording_name(mut self, name: impl Into<String>) -> Self {
        self.recording_info = self.recording_info.with_name(name.into());
        self
    }

    /// Sets an optional name for the recording.
    #[inline]
    pub fn recording_started(mut self, started: impl Into<Timestamp>) -> Self {
        self.recording_info = self.recording_info.with_start_time(started);
        self
    }

    /// Whether the [`RecordingInfo`] chunk should be sent.
    #[inline]
    pub fn send_properties(mut self, should_send: bool) -> Self {
        self.should_send_properties = should_send;
        self
    }

    /// Set a blueprint and make it active immediately.
    ///
    /// Use this when you want your blueprint to be shown immediately.
    ///
    /// To send a blueprint to an existing recording, use [`RecordingStream::send_blueprint`] instead.
    #[inline]
    pub fn with_blueprint(mut self, blueprint: crate::blueprint::Blueprint) -> Self {
        self.blueprint = Some(crate::blueprint::BlueprintOpts {
            blueprint,
            activation: crate::blueprint::BlueprintActivation {
                make_active: true,
                make_default: true,
            },
        });
        self
    }

    /// Set a default blueprint for this application.
    ///
    /// If the application already has an active blueprint, the new blueprint won't become
    /// active until the user resets the blueprint. If you want to activate the blueprint
    /// immediately, use [`Self::with_blueprint`] instead.
    ///
    /// To send a blueprint to an existing recording, use [`RecordingStream::send_blueprint`] instead.
    #[inline]
    pub fn with_default_blueprint(mut self, blueprint: crate::blueprint::Blueprint) -> Self {
        self.blueprint = Some(crate::blueprint::BlueprintOpts {
            blueprint,
            activation: crate::blueprint::BlueprintActivation {
                make_active: false,
                make_default: true,
            },
        });
        self
    }

    /// Specifies the configuration of the internal data batching mechanism.
    ///
    /// If not set, the default configuration for the currently active sink will be used.
    /// Any environment variables as specified on [`ChunkBatcherConfig`] will always override respective settings.
    ///
    /// See [`ChunkBatcher`] & [`ChunkBatcherConfig`] for more information.
    #[inline]
    pub fn batcher_config(mut self, config: ChunkBatcherConfig) -> Self {
        self.batcher_config = Some(config);
        self
    }

    /// Specifies callbacks for the batcher thread.
    ///
    /// See [`ChunkBatcher`] & [`BatcherHooks`] for more information.
    #[inline]
    pub fn batcher_hooks(mut self, hooks: BatcherHooks) -> Self {
        self.batcher_hooks = hooks;
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
        self.create_recording_stream("buffered", || {
            Ok(Box::new(crate::log_sink::BufferedSink::new()))
        })
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
        let rec = self.create_recording_stream("memory", || {
            Ok(Box::new(crate::log_sink::BufferedSink::new()))
        })?;

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

    /// Creates a new [`RecordingStream`] pre-configured to stream data to multiple sinks.
    ///
    /// Currently only supports [`GrpcSink`][grpc_sink] and [`FileSink`][file_sink].
    ///
    /// If the batcher configuration has not been set explicitly or by environment variables,
    /// this will change the batcher configuration to a conservative (less often flushing) mix of
    /// default configurations of the underlying sinks.
    ///
    /// [grpc_sink]: crate::sink::GrpcSink
    /// [file_sink]: crate::sink::FileSink
    pub fn set_sinks(
        self,
        sinks: impl crate::sink::IntoMultiSink,
    ) -> RecordingStreamResult<RecordingStream> {
        self.create_recording_stream("set_sinks", || Ok(Box::new(sinks.into_multi_sink())))
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
        self.connect_grpc_opts(format!(
            "rerun+http://127.0.0.1:{}/proxy",
            re_grpc_server::DEFAULT_SERVER_PORT
        ))
    }

    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// remote Rerun instance.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_app")
    ///     .connect_grpc_opts("rerun+http://127.0.0.1:9876/proxy")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn connect_grpc_opts(
        self,
        url: impl Into<String>,
    ) -> RecordingStreamResult<RecordingStream> {
        self.create_recording_stream("connect_grpc", || {
            let url: String = url.into();
            let re_uri::RedapUri::Proxy(uri) = url.as_str().parse()? else {
                return Err(RecordingStreamError::NotAProxyEndpoint);
            };
            Ok(Box::new(crate::log_sink::GrpcSink::new(uri)))
        })
    }

    #[cfg(feature = "server")]
    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// locally hosted gRPC server.
    ///
    /// The server is hosted on the default IP and port, and may be connected to by any SDK or Viewer
    /// at `rerun+http://127.0.0.1:9876/proxy` or by just running `rerun --connect`.
    ///
    /// To configure the gRPC server's IP and port, use [`Self::serve_grpc_opts`] instead.
    ///
    /// The gRPC server will buffer in memory so that late connecting viewers will still get all the data.
    /// You can control the amount of data buffered by the gRPC server using [`Self::serve_grpc_opts`].
    /// Once the memory limit is reached, the earliest logged data
    /// will be dropped. Static data is never dropped.
    ///
    /// NOTE: When the `RecordingStream` is dropped or disconnected, it will shut down the gRPC server.
    pub fn serve_grpc(self) -> RecordingStreamResult<RecordingStream> {
        use re_grpc_server::ServerOptions;

        self.serve_grpc_opts(
            "0.0.0.0",
            crate::DEFAULT_SERVER_PORT,
            ServerOptions {
                memory_limit: re_memory::MemoryLimit::from_fraction_of_total(0.25),
                ..Default::default()
            },
        )
    }

    #[cfg(feature = "server")]
    /// Creates a new [`RecordingStream`] that is pre-configured to stream the data through to a
    /// locally hosted gRPC server.
    ///
    /// The server is hosted on the given `bind_ip` and `port`, may be connected to by any SDK or Viewer
    /// at `rerun+http://{bind_ip}:{port}/proxy`.
    ///
    /// `0.0.0.0` is a good default for `bind_ip`.
    ///
    /// The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
    /// You can control the amount of data buffered by the gRPC server with the `server_options` argument.
    /// Once reached, the earliest logged data will be dropped. Static data is never dropped.
    ///
    /// NOTE: When the `RecordingStream` is dropped or disconnected, it will shut down the gRPC server.
    pub fn serve_grpc_opts(
        self,
        bind_ip: impl AsRef<str>,
        port: u16,
        server_options: re_grpc_server::ServerOptions,
    ) -> RecordingStreamResult<RecordingStream> {
        self.create_recording_stream("serve_grpc", || {
            Ok(Box::new(crate::grpc_server::GrpcServerSink::new(
                bind_ip.as_ref(),
                port,
                server_options,
            )?))
        })
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
        self.create_recording_stream("save", || Ok(Box::new(crate::sink::FileSink::new(path)?)))
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

        self.create_recording_stream("stdout", || Ok(Box::new(crate::sink::FileSink::stdout()?)))
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
        self.spawn_opts(&Default::default())
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
    ///     .spawn_opts(&re_sdk::SpawnOptions::default())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn spawn_opts(self, opts: &crate::SpawnOptions) -> RecordingStreamResult<RecordingStream> {
        if !self.is_enabled() {
            re_log::debug!("Rerun disabled - call to spawn() ignored");
            return Ok(RecordingStream::disabled());
        }

        let url = format!("rerun+http://{}/proxy", opts.connect_addr());

        // NOTE: If `_RERUN_TEST_FORCE_SAVE` is set, all recording streams will write to disk no matter
        // what, thus spawning a viewer is pointless (and probably not intended).
        if forced_sink_path().is_some() {
            return self.connect_grpc_opts(url);
        }

        // Spawn viewer and connect normally
        crate::spawn(opts)?;

        self.connect_grpc_opts(url)
    }

    /// Returns whether or not logging is enabled, a [`StoreInfo`], the associated batcher
    /// configuration, and the blueprint.
    ///
    /// This can be used to then construct a [`RecordingStream`] manually using
    /// [`RecordingStream::new`].
    pub fn into_args(
        self,
    ) -> (
        bool,
        StoreInfo,
        Option<RecordingInfo>,
        Option<ChunkBatcherConfig>,
        BatcherHooks,
        Option<crate::blueprint::BlueprintOpts>,
    ) {
        let enabled = self.is_enabled();

        let Self {
            application_id,
            store_kind,
            recording_id,
            store_source,
            default_enabled: _,
            enabled: _,
            batcher_config,
            batcher_hooks,
            should_send_properties,
            recording_info,
            blueprint,
        } = self;

        let store_id = StoreId::new(
            store_kind,
            application_id,
            recording_id.unwrap_or_else(RecordingId::random),
        );
        let store_source = store_source.unwrap_or_else(|| StoreSource::RustSdk {
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
        });

        let store_info = StoreInfo::new(store_id, store_source);

        (
            enabled,
            store_info,
            should_send_properties.then_some(recording_info),
            batcher_config,
            batcher_hooks,
            blueprint,
        )
    }

    fn create_recording_stream(
        self,
        function_name: &'static str,
        sink_factory: impl FnOnce() -> RecordingStreamResult<Box<dyn LogSink>>,
    ) -> RecordingStreamResult<RecordingStream> {
        let (enabled, store_info, properties, batcher_config, batcher_hooks, blueprint_opts) =
            self.into_args();
        if enabled {
            let stream = RecordingStream::new(
                store_info,
                properties,
                batcher_config,
                batcher_hooks,
                sink_factory()?,
            )?;
            if let Some(blueprint_opts) = blueprint_opts {
                blueprint_opts.send(&stream)?;
            }
            Ok(stream)
        } else {
            re_log::debug!("Rerun disabled - call to {function_name}() ignored");
            Ok(RecordingStream::disabled())
        }
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
    inner: Either<Arc<RecordingStreamInner>, Weak<RecordingStreamInner>>,
}

impl RecordingStream {
    /// Passes a reference to the [`RecordingStreamInner`], if it exists.
    ///
    /// This works whether the underlying stream is strong or weak.
    #[inline]
    fn with<F: FnOnce(&RecordingStreamInner) -> R, R>(&self, f: F) -> Option<R> {
        match &self.inner {
            Either::Left(strong) => Some(f(strong)),
            Either::Right(weak) => Some(f(&*weak.upgrade()?)),
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

    /// Returns the current reference count of the [`RecordingStream`].
    ///
    /// Returns 0 if the stream was created by [`RecordingStream::disabled()`],
    /// or if it is a [`clone_weak()`][Self::clone_weak] of a stream whose strong instances
    /// have all been dropped.
    pub fn ref_count(&self) -> usize {
        match &self.inner {
            Either::Left(strong) => Arc::strong_count(strong),
            Either::Right(weak) => weak.strong_count(),
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
        if let Either::Left(strong) = &mut self.inner
            && Arc::strong_count(strong) == 1
        {
            // Keep the recording alive until all dataloaders are finished.
            self.with(|inner| inner.wait_for_dataloaders());
        }
    }
}

struct RecordingStreamInner {
    store_info: StoreInfo,
    recording_info: Option<RecordingInfo>,
    tick: AtomicI64,

    /// The one and only entrypoint into the pipeline: this is _never_ cloned nor publicly exposed,
    /// therefore the `Drop` implementation is guaranteed that no more data can come in while it's
    /// running.
    cmds_tx: re_quota_channel::Sender<Command>,

    batcher: ChunkBatcher,
    batcher_to_sink_handle: Option<std::thread::JoinHandle<()>>,

    /// It true, any new sink will update the batcher's configuration (as far as possible).
    sink_dependent_batcher_config: bool,

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
            .field("store_id", &self.store_info.store_id)
            .finish()
    }
}

impl Drop for RecordingStreamInner {
    fn drop(&mut self) {
        if self.is_forked_child() {
            re_log::error_once!(
                "Fork detected while dropping RecordingStreamInner. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK."
            );
            return;
        }

        self.wait_for_dataloaders();

        // NOTE: The command channel is private, if we're here, nothing is currently capable of
        // sending data down the pipeline.
        let timeout = Duration::MAX;
        if let Err(err) = self.batcher.flush_blocking(timeout) {
            re_log::error!("Failed to flush batcher: {err}");
        }
        self.cmds_tx.send(Command::PopPendingChunks).ok();
        self.cmds_tx.send(Command::Shutdown).ok();
        if let Some(handle) = self.batcher_to_sink_handle.take() {
            handle.join().ok();
        }
    }
}

fn resolve_batcher_config(
    batcher_config: Option<ChunkBatcherConfig>,
    sink: &dyn LogSink,
) -> ChunkBatcherConfig {
    if let Some(explicit_batcher_config) = batcher_config {
        explicit_batcher_config
    } else {
        let default_config = sink.default_batcher_config();
        default_config.apply_env().unwrap_or_else(|err| {
            re_log::error!("Failed to parse ChunkBatcherConfig from env: {err}");
            default_config
        })
    }
}

impl RecordingStreamInner {
    fn new(
        store_info: StoreInfo,
        recording_info: Option<RecordingInfo>,
        batcher_config: Option<ChunkBatcherConfig>,
        batcher_hooks: BatcherHooks,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        let sink_dependent_batcher_config = batcher_config.is_none();
        let batcher_config = resolve_batcher_config(batcher_config, &*sink);

        let on_release = batcher_hooks.on_release.clone();
        let batcher = ChunkBatcher::new(batcher_config, batcher_hooks)?;

        {
            re_log::debug!(
                store_id = ?store_info.store_id,
                "Setting StoreInfo",
            );
            sink.send(
                re_log_types::SetStoreInfo {
                    row_id: *RowId::new(),
                    info: store_info.clone(),
                }
                .into(),
            );
        }

        let (cmds_tx, cmds_rx) = re_quota_channel::channel(
            "RecordingStream::cmds",
            batcher_config.max_bytes_in_flight / 2,
        );

        let batcher_to_sink_handle = {
            const NAME: &str = "RecordingStream::batcher_to_sink";
            std::thread::Builder::new()
                .name(NAME.into())
                .spawn({
                    let info = store_info.clone();
                    let batcher = batcher.clone();
                    move || forwarding_thread(info, sink, cmds_rx, batcher.chunks(), on_release)
                })
                .map_err(|err| RecordingStreamError::SpawnThread {
                    name: NAME.into(),
                    err,
                })?
        };

        if let Some(recording_info) = recording_info.as_ref() {
            // We pre-populate the batcher with a chunk the contains the `RecordingInfo`
            // so that these get automatically sent to the sink.

            re_log::trace!(recording_info = ?recording_info, "Adding RecordingInfo to batcher");

            let chunk = Chunk::builder(EntityPath::properties())
                .with_archetype(RowId::new(), TimePoint::default(), recording_info)
                .build()?;

            batcher.push_chunk(chunk);
        }

        Ok(Self {
            store_info,
            recording_info,
            tick: AtomicI64::new(0),
            cmds_tx,
            batcher,
            batcher_to_sink_handle: Some(batcher_to_sink_handle),
            sink_dependent_batcher_config,
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

type InspectSinkFn = Box<dyn FnOnce(&dyn LogSink) + Send + 'static>;

type FlushResult = Result<(), SinkFlushError>;

enum Command {
    RecordMsg(LogMsg),
    SwapSink {
        new_sink: Box<dyn LogSink>,
        timeout: Duration,
    },
    // TODO(#10444): This should go away with more explicit sinks.
    InspectSink(InspectSinkFn),
    Flush {
        on_done: Sender<FlushResult>,
        timeout: Duration,
    },
    PopPendingChunks,
    Shutdown,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RecordMsg(msg) => f.debug_tuple("RecordMsg").field(msg).finish(),
            Self::SwapSink { .. } => f.debug_struct("SwapSink").finish_non_exhaustive(),
            Self::InspectSink(_) => f.debug_tuple("InspectSink").finish_non_exhaustive(),
            Self::Flush { .. } => f.debug_struct("Flush").finish_non_exhaustive(),
            Self::PopPendingChunks => write!(f, "PopPendingChunks"),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

impl re_byte_size::SizeBytes for Command {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::RecordMsg(msg) => msg.heap_size_bytes(),
            Self::SwapSink { .. }
            | Self::InspectSink(_)
            | Self::Flush { .. }
            | Self::PopPendingChunks
            | Self::Shutdown => 0,
        }
    }
}

impl Command {
    fn flush(timeout: Duration) -> (Self, Receiver<FlushResult>) {
        let (on_done, rx) = crossbeam::channel::bounded(1); // oneshot
        (Self::Flush { on_done, timeout }, rx)
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
    /// If no batcher configuration is provided, the default batcher configuration for the sink will be used.
    /// Any environment variables as specified in [`ChunkBatcherConfig`] will always override respective settings.
    ///
    /// See also: [`RecordingStreamBuilder`].
    #[must_use = "Recording will get closed automatically once all instances of this object have been dropped"]
    pub fn new(
        store_info: StoreInfo,
        recording_info: Option<RecordingInfo>,
        batcher_config: Option<ChunkBatcherConfig>,
        batcher_hooks: BatcherHooks,
        sink: Box<dyn LogSink>,
    ) -> RecordingStreamResult<Self> {
        let sink = store_info
            .store_id
            .is_recording()
            .then(forced_sink_path)
            .flatten()
            .map_or(sink, |path| {
                re_log::info!("Forcing FileSink because of env-var {ENV_FORCE_SAVE}={path:?}");
                Box::new(
                    crate::sink::FileSink::new(path)
                        .expect("Failed to create FileSink for forced test path"),
                ) as Box<dyn LogSink>
            });

        let stream = RecordingStreamInner::new(
            store_info,
            recording_info,
            batcher_config,
            batcher_hooks,
            sink,
        )
        .map(|inner| Self {
            inner: Either::Left(Arc::new(inner)),
        })?;

        Ok(stream)
    }

    /// Creates a new no-op [`RecordingStream`] that drops all logging messages, doesn't allocate
    /// any memory and doesn't spawn any threads.
    ///
    /// [`Self::is_enabled`] will return `false`.
    pub const fn disabled() -> Self {
        Self {
            inner: Either::Right(Weak::new()),
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
    /// # Thread Safety
    ///
    /// While [`RecordingStream`] is `Send + Sync` and safe to use from multiple threads,
    /// **avoid calling `log` while holding a [`std::sync::Mutex`]**. The rerun SDK uses
    /// [rayon](https://docs.rs/rayon) internally for parallel processing, and rayon's
    /// work-stealing behavior can cause deadlocks when combined with held mutexes
    /// (see [rayon#592](https://github.com/rayon-rs/rayon/issues/592)).
    ///
    /// ```ignore
    /// // ❌ Don't do this - potential deadlock:
    /// let guard = mutex.lock().unwrap();
    /// stream.log("data", &rerun::Points3D::new(points))?;
    /// drop(guard);
    ///
    /// // ✅ Do this instead - extract data first:
    /// let points = {
    ///     let guard = mutex.lock().unwrap();
    ///     guard.points.clone()
    /// };
    /// stream.log("data", &rerun::Points3D::new(points))?;
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
    /// [`Self::set_time`]/[`Self::set_timepoint`]/etc. APIs.
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
        comp_batches: impl IntoIterator<Item = re_sdk_types::SerializedComponentBatch>,
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
        let update = RecordingInfo::update_fields().with_name(name.into());
        self.log_static(EntityPath::properties(), &update)
    }

    /// Sends the start time of the recording.
    #[inline]
    pub fn send_recording_start_time(
        &self,
        timestamp: impl Into<Timestamp>,
    ) -> RecordingStreamResult<()> {
        let update = RecordingInfo::update_fields().with_start_time(timestamp.into());
        self.log_static(EntityPath::properties(), &update)
    }

    // NOTE: For bw and fw compatibility reasons, we need our logging APIs to be fallible, even
    // though they really aren't at the moment.
    #[expect(clippy::unnecessary_wraps)]
    fn log_serialized_batches_impl(
        &self,
        row_id: RowId,
        entity_path: impl Into<EntityPath>,
        static_: bool,
        comp_batches: impl IntoIterator<Item = re_sdk_types::SerializedComponentBatch>,
    ) -> RecordingStreamResult<()> {
        if !self.is_enabled() {
            return Ok(()); // silently drop the message
        }

        let entity_path = entity_path.into();

        let components: IntMap<_, _> = comp_batches
            .into_iter()
            .map(|comp_batch| (comp_batch.descriptor.component, comp_batch))
            .collect();

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
    #[expect(clippy::fn_params_excessive_bools)] // private function 🤷‍♂️
    fn log_file(
        &self,
        filepath: impl AsRef<std::path::Path>,
        contents: Option<std::borrow::Cow<'_, [u8]>>,
        entity_path_prefix: Option<EntityPath>,
        static_: bool,
        prefer_current_recording: bool,
    ) -> RecordingStreamResult<()> {
        let Some(store_info) = self.store_info().clone() else {
            re_log::warn!(
                "Ignored call to log_file() because RecordingStream has not been properly initialized"
            );
            return Ok(());
        };

        let filepath = filepath.as_ref();
        let has_contents = contents.is_some();

        let (tx, rx) = re_log_channel::log_channel(re_log_channel::LogSource::File {
            path: filepath.into(),
            follow: false,
        });

        let mut settings = crate::DataLoaderSettings {
            application_id: Some(store_info.application_id().clone()),
            recording_id: store_info.recording_id().clone(),
            opened_store_id: None,
            force_store_info: false,
            entity_path_prefix,
            follow: false,
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
                        match msg {
                            re_log_channel::DataSourceMessage::LogMsg(log_msg) => {
                                this.record_msg(log_msg);
                            }
                            unsupported => {
                                re_log::error_once!(
                                    "Ignoring unexpected {} in file",
                                    unsupported.variant_name()
                                );
                            }
                        }
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

#[expect(clippy::needless_pass_by_value)]
fn forwarding_thread(
    store_info: StoreInfo,
    mut sink: Box<dyn LogSink>,
    cmds_rx: re_quota_channel::Receiver<Command>,
    chunks: re_quota_channel::Receiver<Chunk>,
    on_release: Option<ArrowRecordBatchReleaseCallback>,
) {
    /// Returns `true` to indicate that processing can continue; i.e. `false` means immediate
    /// shutdown.
    fn handle_cmd(store_info: &StoreInfo, cmd: Command, sink: &mut Box<dyn LogSink>) -> bool {
        match cmd {
            Command::RecordMsg(msg) => {
                sink.send(msg);
            }
            Command::SwapSink { new_sink, timeout } => {
                re_log::trace!("Swapping sink…");

                let backlog = {
                    // Capture the backlog if it exists.
                    let backlog = sink.drain_backlog();

                    // Flush the underlying sink if possible.
                    if let Err(err) = sink.flush_blocking(timeout) {
                        re_log::error!("Failed to flush previous sink: {err}");
                    }

                    backlog
                };

                // Send the recording info to the new sink. This is idempotent.
                {
                    re_log::debug!(
                        store_id = ?store_info.store_id,
                        "Setting StoreInfo",
                    );
                    new_sink.send(
                        re_log_types::SetStoreInfo {
                            row_id: *RowId::new(),
                            info: store_info.clone(),
                        }
                        .into(),
                    );
                    new_sink.send_all(backlog);
                }

                *sink = new_sink;
            }
            Command::InspectSink(f) => {
                f(sink.as_ref());
            }
            Command::Flush { on_done, timeout } => {
                re_log::trace!("Flushing…");

                let result = sink.flush_blocking(timeout);

                // Send back the result:
                if let Err(crossbeam::channel::SendError(result)) = send_crossbeam(&on_done, result)
                    && let Err(err) = result
                {
                    // There was an error, and nobody received it:
                    re_log::error!("Failed to flush sink: {err}");
                }
            }
            Command::PopPendingChunks => {
                // Wake up and skip the current iteration so that we can drain all pending chunks
                // before handling the next command.
            }
            Command::Shutdown => return false,
        }

        true
    }

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
            sink.send(LogMsg::ArrowMsg(store_info.store_id.clone(), msg));
        }

        re_quota_channel::select! {
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

                sink.send(LogMsg::ArrowMsg(store_info.store_id.clone(), msg));
            }

            recv(cmds_rx) -> res => {
                let Ok(cmd) = res else {
                    // All command senders are gone, which can only happen if the
                    // `RecordingStream` itself has been dropped.
                    re_log::trace!("Shutting down forwarding_thread: all command senders are gone");
                    break;
                };
                if !handle_cmd(&store_info, cmd, &mut sink) {
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
        self.with(|inner| inner.store_info.clone())
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

                let repeated_time = std::iter::repeat_n(time.as_i64(), chunk.num_rows()).collect();

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

                let repeated_tick = std::iter::repeat_n(tick, chunk.num_rows()).collect();

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

    /// Logs multiple [`Chunk`]s.
    ///
    /// This will _not_ inject `log_tick` and `log_time` timeline columns into the chunk,
    /// for that use [`Self::log_chunks`].
    pub fn log_chunks(&self, chunks: impl IntoIterator<Item = Chunk>) {
        for chunk in chunks {
            self.log_chunk(chunk);
        }
    }

    /// Records a single [`Chunk`].
    ///
    /// Will inject `log_tick` and `log_time` timeline columns into the chunk.
    /// If you don't want to inject these, use [`Self::send_chunks`] instead.
    #[inline]
    pub fn send_chunk(&self, chunk: Chunk) {
        let f = move |inner: &RecordingStreamInner| {
            inner.batcher.push_chunk(chunk);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to send_chunk() ignored");
        }
    }

    /// Records multiple [`Chunk`]s.
    ///
    /// This will _not_ inject `log_tick` and `log_time` timeline columns into the chunk,
    /// for that use [`Self::log_chunks`].
    pub fn send_chunks(&self, chunks: impl IntoIterator<Item = Chunk>) {
        for chunk in chunks {
            self.send_chunk(chunk);
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
    /// If the batcher's configuration has not been set explicitly or by environment variables,
    /// this will change the batcher configuration to the sink's default configuration.
    ///
    /// ## Data loss
    ///
    /// If the current sink is in a broken state (e.g. a gRPC sink with a broken connection that
    /// cannot be repaired), all pending data in its buffers will be dropped.
    pub fn set_sink(&self, new_sink: Box<dyn LogSink>) {
        if self.is_forked_child() {
            re_log::error_once!(
                "Fork detected during set_sink. cleanup_if_forked() should always be called after forking. This is likely a bug in the SDK."
            );
            return;
        }

        let timeout = Duration::MAX; // The background thread should block forever if necessary

        let f = move |inner: &RecordingStreamInner| {
            // NOTE: Internal channels can never be closed outside of the `Drop` impl, all these sends
            // are safe.

            // Flush the batcher down the chunk channel
            if let Err(err) = inner.batcher.flush_blocking(timeout) {
                re_log::warn!("Failed to flush batcher in `set_sink`: {err}");
            }

            // Receive pending chunks from the batcher's channel
            inner.cmds_tx.send(Command::PopPendingChunks).ok();

            // Update the batcher's configuration if it's sink-dependent.
            if inner.sink_dependent_batcher_config {
                let batcher_config = resolve_batcher_config(None, &*new_sink);
                inner.batcher.update_config(batcher_config);
            }

            // Swap the sink, which will internally make sure to re-ingest the backlog if needed
            inner
                .cmds_tx
                .send(Command::SwapSink { new_sink, timeout })
                .ok();

            // Before we give control back to the caller, we need to make sure that the swap has
            // taken place: we don't want the user to send data to the old sink!
            re_log::trace!("Waiting for sink swap to complete…");
            let (cmd, oneshot) = Command::flush(timeout);
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
    ///
    /// This will never return [`SinkFlushError::Timeout`].
    pub fn flush_async(&self) -> Result<(), SinkFlushError> {
        re_tracing::profile_function!();
        match self.flush(None) {
            Err(SinkFlushError::Timeout) => Ok(()),
            result => result,
        }
    }

    /// Flush the batching pipeline and waits for it to propagate.
    ///
    /// The function will block until either the flush has completed successfully (`Ok`),
    /// an error has occurred (`SinkFlushError::Failed`), or the timeout is reached (`SinkFlushError::Timeout`).
    ///
    /// Convenience for calling [`Self::flush_with_timeout`] with a timeout of [`Duration::MAX`]
    pub fn flush_blocking(&self) -> Result<(), SinkFlushError> {
        re_tracing::profile_function!();
        self.flush_with_timeout(Duration::MAX)
    }

    /// Flush the batching pipeline and optionally waits for it to propagate.
    /// If you don't want a timeout you can pass in [`Duration::MAX`].
    ///
    /// The function will block until that timeout is reached,
    /// an error occurs, or the flush is complete.
    /// The function will only block while there is some hope of progress.
    /// For instance: if the underlying gRPC connection is disconnected (or never connected at all),
    /// then [`SinkFlushError::Failed`] is returned.
    ///
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    pub fn flush_with_timeout(&self, timeout: Duration) -> Result<(), SinkFlushError> {
        re_tracing::profile_function!();
        self.flush(Some(timeout))
    }

    /// Flush the batching pipeline and optionally waits for it to propagate.
    ///
    /// If `timeout` is `None`, then this function will start the flush, but NOT wait for it to finish.
    ///
    /// If a `timeout` is given, then the function will block until that timeout is reached,
    /// an error occurs, or the flush is complete.
    ///
    /// See [`RecordingStream`] docs for ordering semantics and multithreading guarantees.
    fn flush(&self, timeout: Option<Duration>) -> Result<(), SinkFlushError> {
        if self.is_forked_child() {
            return Err(SinkFlushError::failed(
                "Fork detected during flush. cleanup_if_forked() should always be called after forking. This is likely a bug in the Rerun SDK.",
            ));
        }

        let f = move |inner: &RecordingStreamInner| -> Result<(), SinkFlushError> {
            // 0. Wait for all pending data loader threads to complete
            //
            // This ensures that data from `log_file_from_path` and `log_file_from_contents`
            // is fully loaded before we flush the batcher and sink.
            inner.wait_for_dataloaders();

            // 1. Synchronously flush the batcher down the chunk channel
            //
            // NOTE: This _has_ to be done synchronously as we need to be guaranteed that all chunks
            // are ready to be drained by the time this call returns.
            // It cannot block indefinitely and is fairly fast as it only requires compute (no I/O).
            inner
                .batcher
                .flush_blocking(Duration::MAX)
                .map_err(|err| match err {
                    BatcherFlushError::Closed => SinkFlushError::failed(err.to_string()),
                    BatcherFlushError::Timeout => SinkFlushError::Timeout,
                })?;

            // 2. Drain all pending chunks from the batcher's channel _before_ any other future command
            inner
                .cmds_tx
                .send(Command::PopPendingChunks)
                .map_err(|_ignored| {
                    SinkFlushError::failed(
                        "Sink shut down prematurely. This is likely a bug in the Rerun SDK.",
                    )
                })?;

            // 3. Asynchronously flush everything down the sink
            let (cmd, on_done) = Command::flush(Duration::MAX); // The background thread should block forever if necessary
            inner.cmds_tx.send(cmd).map_err(|_ignored| {
                SinkFlushError::failed(
                    "Sink shut down prematurely. This is likely a bug in the Rerun SDK.",
                )
            })?;

            if let Some(timeout) = timeout {
                on_done.recv_timeout(timeout).map_err(|err| match err {
                    RecvTimeoutError::Timeout => SinkFlushError::Timeout,
                    RecvTimeoutError::Disconnected => SinkFlushError::failed(
                        "Flush never finished. This is likely a bug in the Rerun SDK.",
                    ),
                })??;
            }

            Ok(())
        };

        match self.with(f) {
            Some(Ok(())) => Ok(()),
            Some(Err(err)) => Err(err),
            None => {
                re_log::warn_once!("Recording disabled - call to flush ignored");
                Ok(())
            }
        }
    }
}

impl RecordingStream {
    /// Stream data to multiple different sinks.
    ///
    /// This is semantically the same as calling [`RecordingStream::set_sink`], but the resulting
    /// [`RecordingStream`] will now stream data to multiple sinks at the same time.
    ///
    /// Currently only supports [`GrpcSink`][grpc_sink] and [`FileSink`][file_sink].
    ///
    /// If the batcher's configuration has not been set explicitly or by environment variables,
    /// This will take over a conservative default of the new sinks.
    /// (there's no guarantee on when exactly the new configuration will be active)
    ///
    /// [grpc_sink]: crate::sink::GrpcSink
    /// [file_sink]: crate::sink::FileSink
    pub fn set_sinks(&self, sinks: impl crate::log_sink::IntoMultiSink) {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new MultiSink since {ENV_FORCE_SAVE} is set");
            return;
        }

        let sink = sinks.into_multi_sink();

        self.set_sink(Box::new(sink));
    }

    /// Asynchronously calls a method that has read access to the currently active sink.
    ///
    /// Since a recording stream's sink is owned by a different thread there is no guarantee when
    /// the callback is going to be called.
    /// It's advised to return as quickly as possible from the callback since
    /// as long as the callback doesn't return, the sink will not receive any new data,
    ///
    /// # Experimental
    ///
    /// This is an experimental API and may change in future releases.
    // TODO(#10444): This should become a lot more straight forward with explicit sinks.
    pub fn inspect_sink(&self, f: impl FnOnce(&dyn LogSink) + Send + 'static) {
        self.with(|inner| inner.cmds_tx.send(Command::InspectSink(Box::new(f))).ok());
    }

    /// Swaps the underlying sink for a [`crate::log_sink::GrpcSink`] sink pre-configured to use
    /// the specified address.
    ///
    /// See also [`Self::connect_grpc_opts`] if you wish to configure the connection.
    ///
    /// This is a convenience wrapper for [`Self::set_sink`] that upholds the same guarantees in
    /// terms of data durability and ordering.
    /// See [`Self::set_sink`] for more information.
    pub fn connect_grpc(&self) -> RecordingStreamResult<()> {
        self.connect_grpc_opts(format!(
            "rerun+http://127.0.0.1:{}/proxy",
            re_grpc_server::DEFAULT_SERVER_PORT
        ))
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
    pub fn connect_grpc_opts(&self, url: impl Into<String>) -> RecordingStreamResult<()> {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new GrpcSink since {ENV_FORCE_SAVE} is set");
            return Ok(());
        }

        let url: String = url.into();
        let re_uri::RedapUri::Proxy(uri) = url.as_str().parse()? else {
            return Err(RecordingStreamError::NotAProxyEndpoint);
        };

        let sink = crate::log_sink::GrpcSink::new(uri);

        self.set_sink(Box::new(sink));
        Ok(())
    }

    #[cfg(feature = "server")]
    /// Swaps the underlying sink for a [`crate::grpc_server::GrpcServerSink`] pre-configured to listen on
    /// `rerun+http://127.0.0.1:9876/proxy`.
    ///
    /// To configure the gRPC server's IP and port, use [`Self::serve_grpc_opts`] instead.
    ///
    /// You can connect a viewer to it with `rerun --connect`.
    ///
    /// The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
    /// You can limit the amount of data buffered by the gRPC server with the `server_options` argument.
    /// Once reached, the earliest logged data will be dropped. Static data is never dropped.
    pub fn serve_grpc(
        &self,
        server_options: re_grpc_server::ServerOptions,
    ) -> RecordingStreamResult<()> {
        self.serve_grpc_opts("0.0.0.0", crate::DEFAULT_SERVER_PORT, server_options)
    }

    #[cfg(feature = "server")]
    /// Swaps the underlying sink for a [`crate::grpc_server::GrpcServerSink`] pre-configured to listen on
    /// `rerun+http://{bind_ip}:{port}/proxy`.
    ///
    /// `0.0.0.0` is a good default for `bind_ip`.
    ///
    /// The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
    /// You can limit the amount of data buffered by the gRPC server with the `server_options` argument.
    /// Once reached, the earliest logged data will be dropped. Static data is never dropped.
    pub fn serve_grpc_opts(
        &self,
        bind_ip: impl AsRef<str>,
        port: u16,
        server_options: re_grpc_server::ServerOptions,
    ) -> RecordingStreamResult<()> {
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting GrpcServerSink since {ENV_FORCE_SAVE} is set");
            return Ok(());
        }

        let sink = crate::grpc_server::GrpcServerSink::new(bind_ip.as_ref(), port, server_options)?;

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
        self.spawn_opts(&Default::default())
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
    pub fn spawn_opts(&self, opts: &crate::SpawnOptions) -> RecordingStreamResult<()> {
        if !self.is_enabled() {
            re_log::debug!("Rerun disabled - call to spawn() ignored");
            return Ok(());
        }
        if forced_sink_path().is_some() {
            re_log::debug!("Ignored setting new GrpcSink since {ENV_FORCE_SAVE} is set");
            return Ok(());
        }

        crate::spawn(opts)?;

        self.connect_grpc_opts(format!("rerun+http://{}/proxy", opts.connect_addr()))?;

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
    pub fn binary_stream(&self) -> BinaryStreamStorage {
        let (sink, storage) = crate::sink::BinaryStreamSink::new(self.clone());
        self.set_sink(Box::new(sink));
        storage
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

    /// Send a blueprint through this recording stream.
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
                    "Blueprint ID mismatch when sending blueprint: {:?} != {:?}. Ignoring activation.",
                    blueprint_id,
                    activation_cmd.blueprint_id
                );
            }
        }
    }

    /// Send a [`crate::blueprint::Blueprint`] to configure the viewer layout.
    pub fn send_blueprint_opts(
        &self,
        opts: &crate::blueprint::BlueprintOpts,
    ) -> RecordingStreamResult<()> {
        opts.blueprint.send(self, opts.activation)
    }
}

impl fmt::Debug for RecordingStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let with = |inner: &RecordingStreamInner| {
            let RecordingStreamInner {
                // This pattern match prevents _accidentally_ omitting data from the debug output
                // when new fields are added.
                store_info,
                recording_info,
                tick,
                cmds_tx: _,
                batcher: _,
                batcher_to_sink_handle: _,
                sink_dependent_batcher_config,
                dataloader_handles,
                pid_at_creation,
            } = inner;

            f.debug_struct("RecordingStream")
                .field("store_info", &store_info)
                .field("recording_info", &recording_info)
                .field("tick", &tick)
                .field(
                    "sink_dependent_batcher_config",
                    &sink_dependent_batcher_config,
                )
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
        let f =
            move |inner: &RecordingStreamInner| ThreadInfo::thread_now(&inner.store_info.store_id);
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
    /// - [`Self::set_duration_secs`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_timepoint(&self, timepoint: impl Into<TimePoint>) {
        let f = move |inner: &RecordingStreamInner| {
            let timepoint = timepoint.into();
            for (timeline, time) in timepoint {
                ThreadInfo::set_thread_time(&inner.store_info.store_id, timeline, time);
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
    /// - [`Self::set_duration_secs`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    pub fn set_time(&self, timeline: impl Into<TimelineName>, value: impl TryInto<TimeCell>) {
        let f = move |inner: &RecordingStreamInner| {
            let timeline = timeline.into();
            if let Ok(value) = value.try_into() {
                ThreadInfo::set_thread_time(&inner.store_info.store_id, timeline, value);
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
    /// - [`Self::set_duration_secs`]
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
    /// For example: `rec.set_duration_secs("time_since_start", time_offset)`.
    /// You can remove a timeline again using `rec.disable_timeline("time_since_start")`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time`]
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_timestamp_secs_since_epoch`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    #[inline]
    pub fn set_duration_secs(&self, timeline: impl Into<TimelineName>, secs: impl Into<f64>) {
        let secs = secs.into();
        if let Ok(duration) = std::time::Duration::try_from_secs_f64(secs) {
            self.set_time(timeline, duration);
        } else {
            re_log::error_once!("set_duration_secs: can't set time to {secs}");
        }
    }

    /// Set a timestamp as seconds since Unix epoch (1970-01-01 00:00:00 UTC).
    ///
    /// Short for `self.set_time(timeline, rerun::TimeCell::from_timestamp_secs_since_epoch(secs))`.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
    ///
    /// You can remove a timeline again using `rec.disable_timeline(timeline)`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time`]
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_duration_secs`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_timestamp_nanos_since_epoch`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    #[inline]
    pub fn set_timestamp_secs_since_epoch(
        &self,
        timeline: impl Into<TimelineName>,
        secs: impl Into<f64>,
    ) {
        self.set_time(
            timeline,
            TimeCell::from_timestamp_secs_since_epoch(secs.into()),
        );
    }

    /// Set a timestamp as nanoseconds since Unix epoch (1970-01-01 00:00:00 UTC).
    ///
    /// Short for `self.set_time(timeline, rerun::TimeCell::set_timestamp_nanos_since_epoch(secs))`.
    ///
    /// Used for all subsequent logging performed from this same thread, until the next call
    /// to one of the index/time setting methods.
    ///
    /// You can remove a timeline again using `rec.disable_timeline(timeline)`.
    ///
    /// There is no requirement of monotonicity. You can move the time backwards if you like.
    ///
    /// See also:
    /// - [`Self::set_time`]
    /// - [`Self::set_timepoint`]
    /// - [`Self::set_duration_secs`]
    /// - [`Self::set_time_sequence`]
    /// - [`Self::set_timestamp_secs_since_epoch`]
    /// - [`Self::disable_timeline`]
    /// - [`Self::reset_time`]
    #[inline]
    pub fn set_timestamp_nanos_since_epoch(
        &self,
        timeline: impl Into<TimelineName>,
        nanos: impl Into<i64>,
    ) {
        self.set_time(
            timeline,
            TimeCell::from_timestamp_nanos_since_epoch(nanos.into()),
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
    /// - [`Self::reset_time`]
    pub fn disable_timeline(&self, timeline: impl Into<TimelineName>) {
        let f = move |inner: &RecordingStreamInner| {
            let timeline = timeline.into();
            ThreadInfo::unset_thread_time(&inner.store_info.store_id, &timeline);
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
    /// - [`Self::disable_timeline`]
    pub fn reset_time(&self) {
        let f = move |inner: &RecordingStreamInner| {
            ThreadInfo::reset_thread_time(&inner.store_info.store_id);
        };

        if self.with(f).is_none() {
            re_log::warn_once!("Recording disabled - call to reset_time() ignored");
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;
    use itertools::Itertools as _;
    use re_log_types::example_components::{MyLabel, MyPoints};
    use re_sdk_types::SerializedComponentBatch;

    use super::*;

    struct DisplayDescrs(Chunk);

    impl std::fmt::Debug for DisplayDescrs {
        #[inline]
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_list()
                .entries(
                    self.0
                        .component_descriptors()
                        .map(|d| d.display_name().to_owned())
                        .sorted(),
                )
                .finish()
        }
    }

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

        // Chunk that contains the `RecordingInfo`.
        match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(store_info.store_id, rid);

                let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                chunk.sanity_check().unwrap();

                assert_debug_snapshot!(DisplayDescrs(chunk));
            }
            _ => panic!("expected ArrowMsg"),
        }

        // Final message is the batched chunk itself.
        match msgs.pop().unwrap() {
            LogMsg::ArrowMsg(rid, msg) => {
                assert_eq!(store_info.store_id, rid);

                let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                chunk.sanity_check().unwrap();

                assert_debug_snapshot!(DisplayDescrs(chunk));
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

                assert_debug_snapshot!(DisplayDescrs(chunk));
            }
            _ => panic!("expected ArrowMsg"),
        };

        // 3rd, 4th, 5th, and 6th messages are all the single-row batched chunks themselves,
        // which were sent as a result of the implicit flush when swapping the underlying sink
        // from buffered to in-memory. Note that these messages contain the 2 recording property
        // chunks.
        assert_next_row(); // Contains `RecordingInfo`
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

            // The batch that contains the `RecordingInfo`.
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(store_info.store_id, rid);

                    let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                    chunk.sanity_check().unwrap();

                    assert_debug_snapshot!(DisplayDescrs(chunk));
                }
                _ => panic!("expected ArrowMsg"),
            }

            // The batched chunk itself, which was sent as a result of the explicit flush above.
            match msgs.pop().unwrap() {
                LogMsg::ArrowMsg(rid, msg) => {
                    assert_eq!(store_info.store_id, rid);

                    let chunk = Chunk::from_arrow_msg(&msg).unwrap();

                    chunk.sanity_check().unwrap();

                    assert_debug_snapshot!(DisplayDescrs(chunk));
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

        assert_eq!(rec.ref_count(), 0);

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
        use re_sdk_types::Loggable;

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
                        MyPoints::descriptor_points().component,
                        SerializedComponentBatch::new(
                            <MyPoint as Loggable>::to_arrow([
                                MyPoint::new(10.0, 10.0),
                                MyPoint::new(20.0, 20.0),
                            ])
                            .unwrap(),
                            MyPoints::descriptor_points(),
                        ),
                    ), //
                    (
                        MyPoints::descriptor_colors().component,
                        SerializedComponentBatch::new(
                            <MyColor as Loggable>::to_arrow([MyColor(0x8080_80FF)]).unwrap(),
                            MyPoints::descriptor_colors(),
                        ),
                    ), //
                    (
                        MyPoints::descriptor_labels().component,
                        SerializedComponentBatch::new(
                            <MyLabel as Loggable>::to_arrow([] as [MyLabel; 0]).unwrap(),
                            MyPoints::descriptor_labels(),
                        ),
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
                        MyPoints::descriptor_points().component,
                        SerializedComponentBatch::new(
                            <MyPoint as Loggable>::to_arrow([] as [MyPoint; 0]).unwrap(),
                            MyPoints::descriptor_points(),
                        ),
                    ), //
                    (
                        MyPoints::descriptor_colors().component,
                        SerializedComponentBatch::new(
                            <MyColor as Loggable>::to_arrow([] as [MyColor; 0]).unwrap(),
                            MyPoints::descriptor_colors(),
                        ),
                    ), //
                    (
                        MyPoints::descriptor_labels().component,
                        SerializedComponentBatch::new(
                            <MyLabel as Loggable>::to_arrow([] as [MyLabel; 0]).unwrap(),
                            MyPoints::descriptor_labels(),
                        ),
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
                        MyPoints::descriptor_points().component,
                        SerializedComponentBatch::new(
                            <MyPoint as Loggable>::to_arrow([] as [MyPoint; 0]).unwrap(),
                            MyPoints::descriptor_points(),
                        ),
                    ), //
                    (
                        MyPoints::descriptor_colors().component,
                        SerializedComponentBatch::new(
                            <MyColor as Loggable>::to_arrow([MyColor(0xFFFF_FFFF)]).unwrap(),
                            MyPoints::descriptor_colors(),
                        ),
                    ), //
                    (
                        MyPoints::descriptor_labels().component,
                        SerializedComponentBatch::new(
                            <MyLabel as Loggable>::to_arrow([MyLabel("hey".into())]).unwrap(),
                            MyPoints::descriptor_labels(),
                        ),
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
        use re_sdk_types::ComponentBatch as _;
        rec.log(
            "labels",
            &labels
                .try_serialized(MyPoints::descriptor_labels())
                .unwrap(),
        )
        .unwrap();
    }

    struct BatcherConfigTestSink {
        config: ChunkBatcherConfig,
    }

    impl LogSink for BatcherConfigTestSink {
        fn default_batcher_config(&self) -> ChunkBatcherConfig {
            self.config
        }

        fn send(&self, _msg: LogMsg) {
            // noop
        }

        fn flush_blocking(&self, _timeout: Duration) -> Result<(), SinkFlushError> {
            Ok(())
        }
    }

    struct ScopedEnvVarSet {
        key: &'static str,
    }

    impl ScopedEnvVarSet {
        #[expect(unsafe_code)]
        fn new(key: &'static str, value: &'static str) -> Self {
            // SAFETY: only used in tests.
            unsafe { std::env::set_var(key, value) };
            Self { key }
        }
    }

    impl Drop for ScopedEnvVarSet {
        #[expect(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: only used in tests.
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }

    const CONFIG_CHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

    fn clear_environment() {
        // SAFETY: only used in tests.
        #[expect(unsafe_code)]
        unsafe {
            std::env::remove_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED");
            std::env::remove_var("RERUN_FLUSH_NUM_BYTES");
            std::env::remove_var("RERUN_FLUSH_NUM_ROWS");
            std::env::remove_var("RERUN_FLUSH_TICK_SECS");
            std::env::remove_var("RERUN_MAX_CHUNK_ROWS_IF_UNSORTED");
        }
    }

    #[test]
    fn test_sink_dependent_batcher_config() {
        clear_environment();

        let (tx, rx) = crossbeam::channel::bounded(16);

        let rec = RecordingStreamBuilder::new("rerun_example_test_batcher_config")
            .batcher_hooks(BatcherHooks {
                on_config_change: Some(Arc::new(move |config: &ChunkBatcherConfig| {
                    re_quota_channel::send_crossbeam(&tx, *config).unwrap();
                })),
                ..BatcherHooks::NONE
            })
            .buffered()
            .unwrap();

        let new_config = rx
            .recv_timeout(CONFIG_CHANGE_TIMEOUT)
            .expect("no config change message received within timeout");
        assert_eq!(
            new_config,
            ChunkBatcherConfig::from_env().unwrap(),
            "Buffered sink should uses the config from the environment"
        );

        // Change sink to our custom sink. Will it take over the setting?
        let injected_config = ChunkBatcherConfig {
            flush_tick: std::time::Duration::from_secs(123),
            flush_num_bytes: 123,
            flush_num_rows: 123,
            ..new_config
        };
        rec.set_sink(Box::new(BatcherConfigTestSink {
            config: injected_config,
        }));
        let new_config = rx
            .recv_timeout(CONFIG_CHANGE_TIMEOUT)
            .expect("no config change message received within timeout");

        assert_eq!(new_config, injected_config);

        // Set flush num bytes through env var and set the sink again.
        // check that the env var is respected.
        let _scoped_env_guard = ScopedEnvVarSet::new("RERUN_FLUSH_NUM_BYTES", "456");
        rec.set_sink(Box::new(BatcherConfigTestSink {
            config: injected_config,
        }));
        let new_config = rx
            .recv_timeout(CONFIG_CHANGE_TIMEOUT)
            .expect("no config change message received within timeout");
        assert_eq!(
            new_config,
            ChunkBatcherConfig {
                flush_num_bytes: 456,
                ..injected_config
            },
        );
    }

    #[test]
    fn test_explicit_batcher_config() {
        clear_environment();

        // This environment variable should *not* override the explicit config.
        let _scoped_env_guard = ScopedEnvVarSet::new("RERUN_FLUSH_TICK_SECS", "456");
        let explicit_config = ChunkBatcherConfig {
            flush_tick: std::time::Duration::from_secs(123),
            flush_num_bytes: 123,
            flush_num_rows: 123,
            ..ChunkBatcherConfig::DEFAULT
        };

        let (tx, rx) = crossbeam::channel::bounded(16);
        let rec = RecordingStreamBuilder::new("rerun_example_test_batcher_config")
            .batcher_config(explicit_config)
            .batcher_hooks(BatcherHooks {
                on_config_change: Some(Arc::new(move |config: &ChunkBatcherConfig| {
                    re_quota_channel::send_crossbeam(&tx, *config).unwrap();
                })),
                ..BatcherHooks::NONE
            })
            .buffered()
            .unwrap();

        let new_config = rx
            .recv_timeout(CONFIG_CHANGE_TIMEOUT)
            .expect("no config change message received within timeout");
        assert_eq!(new_config, explicit_config);

        // Changing the sink should have no effect since an explicit config is in place.
        rec.set_sink(Box::new(BatcherConfigTestSink {
            config: ChunkBatcherConfig::ALWAYS,
        }));
        // Don't want to stall the test for CONFIG_CHANGE_TIMEOUT here.
        let new_config_recv_result = rx.recv_timeout(std::time::Duration::from_millis(100));
        assert_eq!(
            new_config_recv_result,
            Err(crossbeam::channel::RecvTimeoutError::Timeout)
        );
    }
}
