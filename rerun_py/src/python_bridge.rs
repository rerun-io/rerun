#![expect(clippy::fn_params_excessive_bools)] // We used named arguments, so this is fine
#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![expect(clippy::too_many_arguments)] // We used named arguments, so this is fine

use std::borrow::Borrow as _;
use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use arrow::array::RecordBatch as ArrowRecordBatch;
use itertools::Itertools as _;
use pyo3::exceptions::{PyKeyboardInterrupt, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use re_auth::oauth::Credentials;
use re_auth::oauth::login_flow::{DeviceCodeFlow, DeviceCodeFlowState};
//use crate::reflection::ComponentDescriptorExt as _;
use re_chunk::ChunkBatcherConfig;
use re_log::ResultExt as _;
use re_log_types::external::re_types_core::reflection::ComponentDescriptorExt as _;
use re_log_types::{BlueprintActivationCommand, EntityPathPart, LogMsg, RecordingId};
use re_sdk::external::re_log_encoding::Encoder;
use re_sdk::sink::{BinaryStreamStorage, CallbackSink, MemorySinkStorage, SinkFlushError};
use re_sdk::time::TimePoint;
use re_sdk::{ComponentDescriptor, EntityPath, RecordingStream, RecordingStreamBuilder, TimeCell};
#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;

// --- FFI ---

trait PyRuntimeErrorExt {
    fn wrap(err: impl std::error::Error, message: impl std::fmt::Display) -> pyo3::PyErr;
}

impl PyRuntimeErrorExt for PyRuntimeError {
    fn wrap(err: impl std::error::Error, message: impl std::fmt::Display) -> pyo3::PyErr {
        Self::new_err(format!("{message}: {err}"))
    }
}

use crate::recording::PyRecording;

// The bridge needs to have complete control over the lifetimes of the individual recordings,
// otherwise all the recording shutdown machinery (which includes deallocating C, Rust and Python
// data and joining a bunch of threads) can end up running at any time depending on what the
// Python GC is doing, which obviously leads to very bad things :tm:.
//
// TODO(#2116): drop unused recordings
fn all_recordings() -> parking_lot::MutexGuard<'static, Vec<RecordingStream>> {
    static ALL_RECORDINGS: OnceLock<parking_lot::Mutex<Vec<RecordingStream>>> = OnceLock::new();
    ALL_RECORDINGS.get_or_init(Default::default).lock()
}

// We separately track orphaned recordings. These have been disconnected and are flushing but
// actually dropping them prior to application exit leads to warning about dropping the
// buffered sink.
fn orphaned_recordings() -> parking_lot::MutexGuard<'static, Vec<RecordingStream>> {
    static ORPHANED_RECORDINGS: OnceLock<parking_lot::Mutex<Vec<RecordingStream>>> =
        OnceLock::new();
    ORPHANED_RECORDINGS.get_or_init(Default::default).lock()
}

type GarbageSender = crossbeam::channel::Sender<ArrowRecordBatch>;
type GarbageReceiver = crossbeam::channel::Receiver<ArrowRecordBatch>;

/// ## Release Callbacks
///
/// When Arrow data gets logged from Python to Rust across FFI, it carries with it a `release`
/// callback (see Arrow spec) that will be run when the data gets dropped.
///
/// This is an issue in this case because running that callback will likely try and grab the GIL,
/// which is something that should only happen at very specific times, else we end up with deadlocks,
/// segfaults, aborts…
///
/// ## The garbage queue
///
/// When a [`re_log_types::LogMsg`] that was logged from Python gets dropped on the Rust side, it will end up
/// in this queue.
///
/// The mere fact that the data still exists in this queue prevents the underlying Arrow refcount
/// to go below one, which in turn prevents the associated FFI `release` callback to run, which
/// avoids the issue mentioned above.
///
/// When the time is right, call [`flush_garbage_queue`] to flush the queue and deallocate all the
/// accumulated data for real.
//
// NOTE: `crossbeam` rather than `std` because we need a `Send` & `Sync` receiver.
static GARBAGE_QUEUE: LazyLock<(GarbageSender, GarbageReceiver)> = LazyLock::new(|| {
    #[expect(clippy::disallowed_methods)] // must be unbounded, or we can deadlock
    crossbeam::channel::unbounded()
});

/// Flushes the [`GARBAGE_QUEUE`], therefore running all the associated FFI `release` callbacks.
///
/// Any time you release the GIL (e.g. `py.allow_threads()`), try to slip in a call to this
/// function so we don't accumulate too much garbage.
fn flush_garbage_queue() {
    while GARBAGE_QUEUE.1.try_recv().is_ok() {
        // Implicitly dropping chunks, therefore triggering their `release` callbacks, therefore
        // triggering the native Python GC.
    }
}

// ---

#[cfg(feature = "web_viewer")]
fn global_web_viewer_server()
-> parking_lot::MutexGuard<'static, Option<re_web_viewer_server::WebViewerServer>> {
    static WEB_HANDLE: OnceLock<parking_lot::Mutex<Option<re_web_viewer_server::WebViewerServer>>> =
        OnceLock::new();
    WEB_HANDLE.get_or_init(Default::default).lock()
}

/// Initialize the performance telemetry stack in a static so it can keep running for the entire
/// lifetime of the SDK.
///
/// It will be dropped and flushed down in [`shutdown`].
#[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry"))]
fn init_perf_telemetry() -> parking_lot::MutexGuard<'static, re_perf_telemetry::Telemetry> {
    static TELEMETRY: OnceLock<parking_lot::Mutex<re_perf_telemetry::Telemetry>> = OnceLock::new();
    TELEMETRY
        .get_or_init(|| {
            // NOTE: We're just parsing the environment, hence the `vec![]` for CLI flags.
            use re_perf_telemetry::external::clap::Parser as _;
            let args = re_perf_telemetry::TelemetryArgs::parse_from::<_, String>(vec![]);

            let runtime = crate::utils::get_tokio_runtime(); // telemetry must be init in a Tokio context
            runtime.block_on(async {
                let telemetry = re_perf_telemetry::Telemetry::init(
                    args,
                    // NOTE: It's a static in this case, so it's never dropped anyhow.
                    re_perf_telemetry::TelemetryDropBehavior::Shutdown,
                )
                // Perf telemetry is a developer tool, it's not compiled into final user builds.
                .expect("could not start perf telemetry");
                parking_lot::Mutex::new(telemetry)
            })
        })
        .lock()
}

/// The python module is called "rerun_bindings".
#[pymodule]
#[pyo3(name = "rerun_bindings")]
fn rerun_bindings(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    if cfg!(feature = "perf_telemetry") && re_log::env_var_is_truthy("TELEMETRY_ENABLED") {
        // TODO(tracing/issues#2499): allow installing multiple tracing sinks (https://github.com/tokio-rs/tracing/issues/2499)
    } else {
        // NOTE: We set up the logging this here because some the inner init methods don't respond too kindly to being
        // called more than once.
        // The SDK should not be as noisy as the CLI, so we set log filter to warning if not specified otherwise.
        re_log::setup_logging_with_filter(&re_log::log_filter_from_env_or_default("warn"));
    }

    // There is always value in setting this, even if `re_perf_telemetry` is disabled. For example,
    // the Rerun versioning headers will automatically pick it up.
    //
    // Safety: anything touching the env is unsafe, tis what it is.
    #[expect(unsafe_code)]
    unsafe {
        std::env::set_var("OTEL_SERVICE_NAME", "rerun-py");
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry"))]
    let _telemetry = init_perf_telemetry();

    // These two components are necessary for imports to work
    m.add_class::<PyMemorySinkStorage>()?;
    m.add_class::<PyRecordingStream>()?;
    m.add_class::<PyBinarySinkStorage>()?;
    m.add_class::<PyFileSink>()?;
    m.add_class::<PyGrpcSink>()?;
    m.add_class::<PyComponentDescriptor>()?;
    m.add_class::<PyChunkBatcherConfig>()?;
    m.add_class::<PyDeviceCodeFlow>()?;
    m.add_class::<PyCredentials>()?;

    // If this is a special RERUN_APP_ONLY context (launched via .spawn), we
    // can bypass everything else, which keeps us from preparing an SDK session
    // that never gets used.
    if matches!(std::env::var("RERUN_APP_ONLY").as_deref(), Ok("true")) {
        return Ok(());
    }

    m.add_function(wrap_pyfunction!(get_credentials, m)?)?;
    m.add_function(wrap_pyfunction!(init_login_flow, m)?)?;

    // init
    m.add_function(wrap_pyfunction!(new_recording, m)?)?;
    m.add_function(wrap_pyfunction!(new_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;
    m.add_function(wrap_pyfunction!(cleanup_if_forked_child, m)?)?;
    m.add_function(wrap_pyfunction!(spawn, m)?)?;
    m.add_function(wrap_pyfunction!(flush_and_cleanup_orphaned_recordings, m)?)?;

    // recordings
    m.add_function(wrap_pyfunction!(get_application_id, m)?)?;
    m.add_function(wrap_pyfunction!(get_recording_id, m)?)?;
    m.add_function(wrap_pyfunction!(get_data_recording, m)?)?;
    m.add_function(wrap_pyfunction!(get_global_data_recording, m)?)?;
    m.add_function(wrap_pyfunction!(set_global_data_recording, m)?)?;
    m.add_function(wrap_pyfunction!(get_thread_local_data_recording, m)?)?;
    m.add_function(wrap_pyfunction!(set_thread_local_data_recording, m)?)?;
    m.add_function(wrap_pyfunction!(get_blueprint_recording, m)?)?;
    m.add_function(wrap_pyfunction!(get_global_blueprint_recording, m)?)?;
    m.add_function(wrap_pyfunction!(set_global_blueprint_recording, m)?)?;
    m.add_function(wrap_pyfunction!(get_thread_local_blueprint_recording, m)?)?;
    m.add_function(wrap_pyfunction!(set_thread_local_blueprint_recording, m)?)?;
    m.add_function(wrap_pyfunction!(disconnect_orphaned_recordings, m)?)?;
    m.add_function(wrap_pyfunction!(check_for_rrd_footer, m)?)?;

    // sinks
    m.add_function(wrap_pyfunction!(is_enabled, m)?)?;
    m.add_function(wrap_pyfunction!(binary_stream, m)?)?;
    m.add_function(wrap_pyfunction!(set_sinks, m)?)?;
    m.add_function(wrap_pyfunction!(connect_grpc, m)?)?;
    m.add_function(wrap_pyfunction!(connect_grpc_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(save, m)?)?;
    m.add_function(wrap_pyfunction!(save_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(stdout, m)?)?;
    m.add_function(wrap_pyfunction!(memory_recording, m)?)?;
    m.add_function(wrap_pyfunction!(set_callback_sink, m)?)?;
    m.add_function(wrap_pyfunction!(set_callback_sink_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(serve_grpc, m)?)?;
    m.add_function(wrap_pyfunction!(serve_web_viewer, m)?)?;
    m.add_function(wrap_pyfunction!(serve_web, m)?)?;
    m.add_function(wrap_pyfunction!(disconnect, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;

    // time
    m.add_function(wrap_pyfunction!(set_time_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_duration_nanos, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_timestamp_nanos_since_epoch, m)?)?;
    m.add_function(wrap_pyfunction!(disable_timeline, m)?)?;
    m.add_function(wrap_pyfunction!(reset_time, m)?)?;

    // arrow helpers
    m.add_function(wrap_pyfunction!(crate::arrow::build_fixed_size_list_array, m)?)?;

    // log any
    m.add_function(wrap_pyfunction!(log_arrow_msg, m)?)?;
    m.add_function(wrap_pyfunction!(log_file_from_path, m)?)?;
    m.add_function(wrap_pyfunction!(log_file_from_contents, m)?)?;
    m.add_function(wrap_pyfunction!(send_arrow_chunk, m)?)?;
    m.add_function(wrap_pyfunction!(send_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(send_recording, m)?)?;

    // misc
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(is_dev_build, m)?)?;
    m.add_function(wrap_pyfunction!(get_app_url, m)?)?;
    m.add_function(wrap_pyfunction!(start_web_viewer_server, m)?)?;
    m.add_function(wrap_pyfunction!(escape_entity_path_part, m)?)?;
    m.add_function(wrap_pyfunction!(new_entity_path, m)?)?;

    // properties
    m.add_function(wrap_pyfunction!(new_property_entity_path, m)?)?;
    m.add_function(wrap_pyfunction!(send_recording_name, m)?)?;
    m.add_function(wrap_pyfunction!(send_recording_start_time_nanos, m)?)?;

    use crate::video::asset_video_read_frame_timestamps_nanos;
    m.add_function(wrap_pyfunction!(
        asset_video_read_frame_timestamps_nanos,
        m
    )?)?;

    // recording
    crate::recording::register(m)?;

    // catalog
    crate::catalog::register(py, m)?;

    // viewer
    crate::viewer::register(py, m)?;

    // server
    crate::server::register(py, m)?;

    // urdf
    crate::urdf::register(py, m)?;

    Ok(())
}

// --- Init ---
/// Flush and then cleanup any orphaned recordings.
#[pyfunction]
fn flush_and_cleanup_orphaned_recordings(py: Python<'_>) -> PyResult<()> {
    // Start by clearing the current global data recording. Otherwise this holds
    // a reference to the recording, which prevents it from being dropped.
    set_global_data_recording(py, None);

    py.allow_threads(|| -> Result<(), SinkFlushError> {
        // Now flush all recordings to handle weird cases where the data in the queue
        // is actually holding onto the ref to the recording.
        for recording in all_recordings().iter().chain(orphaned_recordings().iter()) {
            recording.flush_blocking()?;
        }

        // Flush the garbage queue.
        flush_garbage_queue();

        // Finally remove any recordings that have a refcount of 1, which means they are ONLY
        // referenced by the `all_recordings` list and thus can't be referred to by the Python SDK.
        all_recordings().retain(|recording| recording.ref_count() > 1);
        orphaned_recordings().retain(|recording| recording.ref_count() > 1);

        Ok(())
    })
    .map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

/// Disconnect any orphaned recordings.
///
/// This can be used to make sure that recordings get closed/finalized
/// properly when all references have been dropped.
#[pyfunction]
fn disconnect_orphaned_recordings(py: Python<'_>) -> PyResult<()> {
    py.allow_threads(|| -> Result<(), SinkFlushError> {
        // Disconnect any recordings that have a refcount of 1. This means they are
        // only referenced by the `all_recordings` list and thus can't be referred to by the Python SDK.
        let mut orphaned = Vec::new();
        all_recordings().retain(|recording| {
            if recording.ref_count() <= 1 {
                re_log::debug!(
                    "Disconnecting orphaned recording: {}",
                    recording
                        .store_info()
                        .map(|info| info.recording_id().to_string())
                        .unwrap_or_else(|| "<unknown>".to_owned())
                );
                recording.disconnect();
                orphaned.push(recording.clone());
                false
            } else {
                true
            }
        });
        orphaned_recordings().extend(orphaned);

        Ok(())
    })
    .map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

#[derive(FromPyObject)]
enum DurationLike {
    Int(i64),
    Float(f64),
    Duration(Duration),
}

impl DurationLike {
    fn into_duration(self) -> Duration {
        match self {
            Self::Int(i) => Duration::from_secs(i as u64),
            Self::Float(f) => match duration_from_sec(f) {
                Ok(duration) => duration,
                Err(err) => {
                    re_log::error_once!("{err}");
                    Duration::ZERO
                }
            },
            Self::Duration(d) => d,
        }
    }
}

/// Defines the different batching thresholds used within the RecordingStream.
#[pyclass(
    eq,
    name = "ChunkBatcherConfig",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, PartialEq, Eq)]
pub struct PyChunkBatcherConfig(ChunkBatcherConfig);

#[pymethods]
impl PyChunkBatcherConfig {
    #[new]
    #[pyo3(signature = (flush_tick=None, flush_num_bytes=None, flush_num_rows=None, chunk_max_rows_if_unsorted=None))]
    #[pyo3(
        text_signature = "(self, flush_tick=None, flush_num_bytes=None, flush_num_rows=None, chunk_max_rows_if_unsorted=None)"
    )]
    /// Initialize the chunk batcher configuration.
    ///
    /// Check out <https://rerun.io/docs/reference/sdk/micro-batching> for more information.
    ///
    /// Parameters
    /// ----------
    /// flush_tick : int | float | timedelta | None
    ///     Duration of the periodic tick, by default `None`.
    ///     Equivalent to setting: `RERUN_FLUSH_TICK_SECS` environment variable.
    ///
    /// flush_num_bytes : int | None
    ///     Flush if the accumulated payload has a size in bytes equal or greater than this, by default `None`.
    ///     Equivalent to setting: `RERUN_FLUSH_NUM_BYTES` environment variable.
    ///
    /// flush_num_rows : int | None
    ///     Flush if the accumulated payload has a number of rows equal or greater than this, by default `None`.
    ///     Equivalent to setting: `RERUN_FLUSH_NUM_ROWS` environment variable.
    ///
    /// chunk_max_rows_if_unsorted : int | None
    ///     Split a chunk if it contains >= rows than this threshold and one or more of its timelines are unsorted,
    ///     by default `None`.
    ///     Equivalent to setting: `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED` environment variable.
    fn new(
        flush_tick: Option<DurationLike>,
        flush_num_bytes: Option<u64>,
        flush_num_rows: Option<u64>,
        chunk_max_rows_if_unsorted: Option<u64>,
    ) -> Self {
        let default = ChunkBatcherConfig::from_env().unwrap_or_else(|_| {
            re_log::warn!(
                "couldn't init ChunkBatcherConfig from environment, falling back to defaults"
            );
            ChunkBatcherConfig::DEFAULT
        });

        Self(ChunkBatcherConfig {
            flush_tick: flush_tick
                .map(DurationLike::into_duration)
                .unwrap_or(default.flush_tick),
            flush_num_bytes: flush_num_bytes.unwrap_or(default.flush_num_bytes),
            flush_num_rows: flush_num_rows.unwrap_or(default.flush_num_rows),
            chunk_max_rows_if_unsorted: chunk_max_rows_if_unsorted
                .unwrap_or(default.chunk_max_rows_if_unsorted),
            ..default
        })
    }

    #[getter]
    /// Duration of the periodic tick.
    ///
    /// Equivalent to setting: `RERUN_FLUSH_TICK_SECS` environment variable.
    fn get_flush_tick(&self) -> Duration {
        self.0.flush_tick
    }

    #[setter]
    /// Duration of the periodic tick.
    ///
    /// Equivalent to setting: `RERUN_FLUSH_TICK_SECS` environment variable.
    fn set_flush_tick(&mut self, flush_tick: DurationLike) {
        self.0.flush_tick = flush_tick.into_duration();
    }

    #[getter]
    /// Flush if the accumulated payload has a size in bytes equal or greater than this.
    ///
    /// Equivalent to setting: `RERUN_FLUSH_NUM_BYTES` environment variable.
    fn get_flush_num_bytes(&self) -> u64 {
        self.0.flush_num_bytes
    }

    #[setter]
    /// Flush if the accumulated payload has a size in bytes equal or greater than this.
    ///
    /// Equivalent to setting: `RERUN_FLUSH_NUM_BYTES` environment variable.
    fn set_flush_num_bytes(&mut self, flush_num_bytes: u64) {
        self.0.flush_num_bytes = flush_num_bytes;
    }

    #[getter]
    /// Flush if the accumulated payload has a number of rows equal or greater than this.
    ///
    /// Equivalent to setting: `RERUN_FLUSH_NUM_ROWS` environment variable.
    fn get_flush_num_rows(&self) -> u64 {
        self.0.flush_num_rows
    }

    #[setter]
    /// Flush if the accumulated payload has a number of rows equal or greater than this.
    ///
    /// Equivalent to setting: `RERUN_FLUSH_NUM_ROWS` environment variable.
    fn set_flush_num_rows(&mut self, flush_num_rows: u64) {
        self.0.flush_num_rows = flush_num_rows;
    }

    #[getter]
    /// Split a chunk if it contains >= rows than this threshold and one or more of its timelines are unsorted.
    ///
    /// Equivalent to setting: `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED` environment variable.
    fn get_chunk_max_rows_if_unsorted(&self) -> u64 {
        self.0.chunk_max_rows_if_unsorted
    }

    #[setter]
    /// Split a chunk if it contains >= rows than this threshold and one or more of its timelines are unsorted.
    ///
    /// Equivalent to setting: `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED` environment variable.
    fn set_chunk_max_rows_if_unsorted(&mut self, chunk_max_rows_if_unsorted: u64) {
        self.0.chunk_max_rows_if_unsorted = chunk_max_rows_if_unsorted;
    }

    #[expect(non_snake_case)]
    #[staticmethod]
    /// Default configuration, applicable to most use cases.
    fn DEFAULT() -> Self {
        Self(ChunkBatcherConfig::DEFAULT)
    }

    #[expect(non_snake_case)]
    #[staticmethod]
    /// Low-latency configuration, preferred when streaming directly to a viewer.
    fn LOW_LATENCY() -> Self {
        Self(ChunkBatcherConfig::LOW_LATENCY)
    }

    #[expect(non_snake_case)]
    #[staticmethod]
    /// Always flushes ASAP.
    fn ALWAYS() -> Self {
        Self(ChunkBatcherConfig::ALWAYS)
    }

    #[expect(non_snake_case)]
    #[staticmethod]
    /// Never flushes unless manually told to (or hitting one the builtin invariants).
    fn NEVER() -> Self {
        Self(ChunkBatcherConfig::NEVER)
    }

    pub fn __str__(&self) -> String {
        format!("{:#?}", self.0)
    }
}

/// Create a new recording stream.
#[expect(clippy::fn_params_excessive_bools)]
#[allow(clippy::allow_attributes, clippy::struct_excessive_bools)]
#[pyfunction]
#[pyo3(signature = (
    application_id,
    recording_id=None,
    make_default=true,
    make_thread_default=true,
    default_enabled=true,
    send_properties=true,
    batcher_config=None,
))]
fn new_recording(
    py: Python<'_>,
    application_id: String,
    recording_id: Option<String>,
    make_default: bool,
    make_thread_default: bool,
    default_enabled: bool,
    send_properties: bool,
    batcher_config: Option<PyChunkBatcherConfig>,
) -> PyResult<PyRecordingStream> {
    let recording_id = if let Some(recording_id) = recording_id {
        RecordingId::from(recording_id)
    } else {
        default_recording_id(py, &application_id)
    };

    let mut hooks = re_chunk::BatcherHooks::NONE;
    let on_release = |chunk| {
        GARBAGE_QUEUE.0.send(chunk).ok();
    };
    hooks.on_release = Some(on_release.into());

    let mut builder = RecordingStreamBuilder::new(application_id)
        .batcher_hooks(hooks)
        .recording_id(recording_id.clone())
        .store_source(re_log_types::StoreSource::PythonSdk(python_version(py)))
        .default_enabled(default_enabled)
        .send_properties(send_properties);

    if let Some(batcher_config) = batcher_config {
        builder = builder.batcher_config(batcher_config.0);
    }

    let recording = builder
        .buffered()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    if make_default {
        set_global_data_recording(
            py,
            Some(&PyRecordingStream(recording.clone() /* shallow */)),
        );
    }
    if make_thread_default {
        set_thread_local_data_recording(
            py,
            Some(&PyRecordingStream(recording.clone() /* shallow */)),
        );
    }

    // NOTE: The Rust-side of the bindings must be in control of the lifetimes of the recordings!
    all_recordings().push(recording.clone());

    Ok(PyRecordingStream(recording))
}

/// Create a new blueprint stream.
#[expect(clippy::fn_params_excessive_bools)]
#[pyfunction]
#[pyo3(signature = (
    application_id,
    make_default=true,
    make_thread_default=true,
    default_enabled=true,
))]
fn new_blueprint(
    py: Python<'_>,
    application_id: String,
    make_default: bool,
    make_thread_default: bool,
    default_enabled: bool,
) -> PyResult<PyRecordingStream> {
    let mut hooks = re_chunk::BatcherHooks::NONE;
    let on_release = |chunk| {
        GARBAGE_QUEUE.0.send(chunk).ok();
    };
    hooks.on_release = Some(on_release.into());

    let blueprint = RecordingStreamBuilder::new(application_id)
        // We don't currently support additive blueprints, so we should always be generating a new,
        // unique blueprint recording id to avoid collisions.
        .recording_id(RecordingId::random())
        .blueprint()
        .batcher_hooks(hooks)
        .store_source(re_log_types::StoreSource::PythonSdk(python_version(py)))
        .default_enabled(default_enabled)
        .buffered()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    if make_default {
        set_global_blueprint_recording(
            py,
            Some(&PyRecordingStream(blueprint.clone() /* shallow */)),
        );
    }
    if make_thread_default {
        set_thread_local_blueprint_recording(
            py,
            Some(&PyRecordingStream(blueprint.clone() /* shallow */)),
        );
    }

    // NOTE: The Rust-side of the bindings must be in control of the lifetimes of the recordings!
    all_recordings().push(blueprint.clone());

    Ok(PyRecordingStream(blueprint))
}

/// Shutdown the Rerun SDK.
#[pyfunction]
fn shutdown(py: Python<'_>) {
    re_log::debug!("Shutting down the Rerun SDK");
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        // NOTE: Do **NOT** try and drain() `all_recordings` here.
        //
        // Doing so would drop the last remaining reference to these recordings, and therefore
        // trigger their deallocation as well as the deallocation of all the Python and C++ data
        // that they might transitively reference, but this is _NOT_ the right place to do so.
        // This method is called automatically during shutdown via python's `atexit`, which is not
        // a safepoint for deallocating these things, quite far from it.
        //
        // Calling `disconnect()` will already take care of flushing everything that can be flushed,
        // and cleaning up everything that can be safely cleaned up, anyhow.
        // Whatever's left can wait for the OS to clean it up.
        for recording in all_recordings().iter() {
            recording.disconnect();
        }

        flush_garbage_queue();
    });

    #[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry"))]
    init_perf_telemetry().shutdown();
}

// --- Recordings ---

#[pyclass(frozen, module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
#[derive(Clone)]
struct PyRecordingStream(RecordingStream);

#[pymethods]
impl PyRecordingStream {
    /// Determine if this stream is operating in the context of a forked child process.
    ///
    /// This means the stream was created in the parent process. It now exists in the child
    /// process by way of fork, but it is effectively a zombie since its batcher and sink
    /// threads would not have been copied.
    ///
    /// Calling operations such as flush or set_sink will result in an error.
    fn is_forked_child(&self) -> bool {
        self.0.is_forked_child()
    }

    pub fn ref_count(&self) -> usize {
        self.0.ref_count()
    }

    pub fn __str__(&self) -> String {
        format!("RecordingStream({:#?})", self.0.store_info())
    }
}

impl std::ops::Deref for PyRecordingStream {
    type Target = RecordingStream;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Get the current recording stream's application ID.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn get_application_id(recording: Option<&PyRecordingStream>) -> Option<String> {
    get_data_recording(recording)?
        .store_info()
        .map(|info| info.application_id().to_string())
}

/// Get the current recording stream's recording ID.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn get_recording_id(recording: Option<&PyRecordingStream>) -> Option<String> {
    get_data_recording(recording)?
        .store_info()
        .map(|info| info.store_id.recording_id().to_string())
}

/// Returns the currently active data recording in the global scope, if any; fallbacks to the specified recording otherwise, if any.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn get_data_recording(recording: Option<&PyRecordingStream>) -> Option<PyRecordingStream> {
    RecordingStream::get_quiet(
        re_sdk::StoreKind::Recording,
        recording.map(|rec| rec.0.clone()),
    )
    .map(PyRecordingStream)
}

/// Returns the currently active data recording in the global scope, if any.
#[pyfunction]
fn get_global_data_recording() -> Option<PyRecordingStream> {
    RecordingStream::global(re_sdk::StoreKind::Recording).map(PyRecordingStream)
}

/// Cleans up internal state if this is the child of a forked process.
#[pyfunction]
fn cleanup_if_forked_child() {
    re_sdk::cleanup_if_forked_child();
}

/// Replaces the currently active recording in the global scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn set_global_data_recording(
    py: Python<'_>,
    recording: Option<&PyRecordingStream>,
) -> Option<PyRecordingStream> {
    // Swapping the active data recording might drop the refcount of the currently active recording
    // to zero, which means dropping it, which means flushing it, which potentially means
    // deallocating python-owned data, which means grabbing the GIL, thus we need to release the
    // GIL first.
    //
    // NOTE: This cannot happen anymore with the new `ALL_RECORDINGS` thingy, but better safe than
    // sorry.
    py.allow_threads(|| {
        let rec = RecordingStream::set_global(
            re_sdk::StoreKind::Recording,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream);
        flush_garbage_queue();
        rec
    })
}

/// Returns the currently active data recording in the thread-local scope, if any.
#[pyfunction]
fn get_thread_local_data_recording() -> Option<PyRecordingStream> {
    RecordingStream::thread_local(re_sdk::StoreKind::Recording).map(PyRecordingStream)
}

/// Replaces the currently active recording in the thread-local scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn set_thread_local_data_recording(
    py: Python<'_>,
    recording: Option<&PyRecordingStream>,
) -> Option<PyRecordingStream> {
    // Swapping the active data recording might drop the refcount of the currently active recording
    // to zero, which means dropping it, which means flushing it, which potentially means
    // deallocating python-owned data, which means grabbing the GIL, thus we need to release the
    // GIL first.
    //
    // NOTE: This cannot happen anymore with the new `ALL_RECORDINGS` thingy, but better safe than
    // sorry.
    py.allow_threads(|| {
        let rec = RecordingStream::set_thread_local(
            re_sdk::StoreKind::Recording,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream);
        flush_garbage_queue();
        rec
    })
}

/// Returns the currently active blueprint recording in the global scope, if any; fallbacks to the specified recording otherwise, if any.
#[pyfunction]
#[pyo3(signature = (overrides=None))]
fn get_blueprint_recording(overrides: Option<&PyRecordingStream>) -> Option<PyRecordingStream> {
    RecordingStream::get_quiet(
        re_sdk::StoreKind::Blueprint,
        overrides.map(|rec| rec.0.clone()),
    )
    .map(PyRecordingStream)
}

/// Returns the currently active blueprint recording in the global scope, if any.
#[pyfunction]
fn get_global_blueprint_recording() -> Option<PyRecordingStream> {
    RecordingStream::global(re_sdk::StoreKind::Blueprint).map(PyRecordingStream)
}

/// Replaces the currently active recording in the global scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn set_global_blueprint_recording(
    py: Python<'_>,
    recording: Option<&PyRecordingStream>,
) -> Option<PyRecordingStream> {
    // Swapping the active blueprint recording might drop the refcount of the currently active recording
    // to zero, which means dropping it, which means flushing it, which potentially means
    // deallocating python-owned blueprint, which means grabbing the GIL, thus we need to release the
    // GIL first.
    //
    // NOTE: This cannot happen anymore with the new `ALL_RECORDINGS` thingy, but better safe than
    // sorry.
    py.allow_threads(|| {
        let rec = RecordingStream::set_global(
            re_sdk::StoreKind::Blueprint,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream);
        flush_garbage_queue();
        rec
    })
}

/// Returns the currently active blueprint recording in the thread-local scope, if any.
#[pyfunction]
fn get_thread_local_blueprint_recording() -> Option<PyRecordingStream> {
    RecordingStream::thread_local(re_sdk::StoreKind::Blueprint).map(PyRecordingStream)
}

/// Replaces the currently active recording in the thread-local scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn set_thread_local_blueprint_recording(
    py: Python<'_>,
    recording: Option<&PyRecordingStream>,
) -> Option<PyRecordingStream> {
    // Swapping the active blueprint recording might drop the refcount of the currently active recording
    // to zero, which means dropping it, which means flushing it, which potentially means
    // deallocating python-owned blueprint, which means grabbing the GIL, thus we need to release the
    // GIL first.
    //
    // NOTE: This cannot happen anymore with the new `ALL_RECORDINGS` thingy, but better safe than
    // sorry.
    py.allow_threads(|| {
        let rec = RecordingStream::set_thread_local(
            re_sdk::StoreKind::Blueprint,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream);
        flush_garbage_queue();
        rec
    })
}

// --- Sinks ---

/// Whether the recording stream enabled.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn is_enabled(recording: Option<&PyRecordingStream>) -> bool {
    get_data_recording(recording).is_some_and(|rec| rec.is_enabled())
}

/// Helper for forwarding the blueprint memory-sink representation to a given sink
fn send_mem_sink_as_default_blueprint(
    sink: &dyn re_sdk::sink::LogSink,
    default_blueprint: &PyMemorySinkStorage,
) {
    if let Some(id) = default_blueprint.inner.store_id() {
        let activate_cmd = BlueprintActivationCommand::make_default(id);
        sink.send_blueprint(default_blueprint.inner.take(), activate_cmd);
    } else {
        re_log::warn!("Provided `default_blueprint` has no store info, cannot send it.");
    }
}

/// Spawn a new viewer.
#[pyfunction]
#[pyo3(signature = (
    port = 9876,
    memory_limit = "75%".to_owned(),
    server_memory_limit = "1GiB".to_owned(),
    hide_welcome_screen = false,
    detach_process = true,
    executable_name = "rerun".to_owned(),
    executable_path = None,
    extra_args = vec![],
    extra_env = vec![],
))]
fn spawn(
    port: u16,
    memory_limit: String,
    server_memory_limit: String,
    hide_welcome_screen: bool,
    detach_process: bool,
    executable_name: String,
    executable_path: Option<String>,
    extra_args: Vec<String>,
    extra_env: Vec<(String, String)>,
) -> PyResult<()> {
    let spawn_opts = re_sdk::SpawnOptions {
        port,
        wait_for_bind: true,
        memory_limit,
        server_memory_limit,
        hide_welcome_screen,
        detach_process,
        executable_name,
        executable_path,
        extra_args,
        extra_env,
    };

    re_sdk::spawn(&spawn_opts).map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

#[pyclass(
    frozen,
    eq,
    hash,
    name = "GrpcSink",
    module = "rerun_bindings.rerun_bindings"
)]
struct PyGrpcSink {
    uri: re_uri::ProxyUri,
}

impl PartialEq for PyGrpcSink {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

impl std::hash::Hash for PyGrpcSink {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uri.hash(state);
    }
}

#[pymethods]
impl PyGrpcSink {
    /// Initialize a gRPC sink.
    #[new]
    #[pyo3(signature = (url=None))]
    #[pyo3(text_signature = "(self, url=None)")]
    fn new(url: Option<String>) -> PyResult<Self> {
        let url = url.unwrap_or_else(|| re_sdk::DEFAULT_CONNECT_URL.to_owned());
        let uri = url
            .parse::<re_uri::ProxyUri>()
            .map_err(|err| PyRuntimeError::wrap(err, format!("invalid endpoint {url:?}")))?;

        Ok(Self { uri })
    }

    pub fn __repr__(&self) -> String {
        format!("GrpcSink({:#?})", self.uri)
    }
}

#[pyclass(
    frozen,
    eq,
    hash,
    name = "FileSink",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(PartialEq, Hash)]
struct PyFileSink {
    path: PathBuf,
}

#[pymethods]
impl PyFileSink {
    #[new]
    #[pyo3(signature = (path))]
    #[pyo3(text_signature = "(self, path)")]
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn __repr__(&self) -> String {
        format!("FileSink({:#?})", self.path)
    }
}

/// Stream data to multiple sinks.
#[pyfunction]
#[pyo3(signature = (sinks, default_blueprint=None, recording=None))]
fn set_sinks(
    sinks: Vec<PyObject>,
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    if re_sdk::forced_sink_path().is_some() {
        re_log::debug!("Ignored call to `set_sinks()` since _RERUN_TEST_FORCE_SAVE is set");
        return Ok(());
    }

    let mut resolved_sinks: Vec<Box<dyn re_sdk::sink::LogSink>> = Vec::new();
    for sink in sinks {
        if let Ok(sink) = sink.downcast_bound::<PyGrpcSink>(py) {
            let sink = sink.get();
            let sink = re_sdk::sink::GrpcSink::new(sink.uri.clone());
            resolved_sinks.push(Box::new(sink));
        } else if let Ok(sink) = sink.downcast_bound::<PyFileSink>(py) {
            let sink = sink.get();
            let sink = re_sdk::sink::FileSink::new(sink.path.clone())
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            resolved_sinks.push(Box::new(sink));
        } else {
            let type_name = sink.bind(py).get_type().name()?;
            return Err(PyRuntimeError::new_err(format!(
                "{type_name} is not a valid LogSink, must be one of: GrpcSink, FileSink"
            )));
        }
    }

    py.allow_threads(|| {
        let sink = re_sdk::sink::MultiSink::new(resolved_sinks);

        if let Some(default_blueprint) = default_blueprint {
            send_mem_sink_as_default_blueprint(&sink, default_blueprint);
        }

        recording.set_sink(Box::new(sink));

        flush_garbage_queue();
    });

    Ok(())
}

/// Connect the recording stream to a remote Rerun Viewer on the given URL.
#[pyfunction]
#[pyo3(signature = (url, default_blueprint = None, recording = None))]
fn connect_grpc(
    url: Option<String>,
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let url = url.unwrap_or_else(|| re_sdk::DEFAULT_CONNECT_URL.to_owned());
    let uri = url
        .parse::<re_uri::ProxyUri>()
        .map_err(|err| PyRuntimeError::wrap(err, format!("invalid endpoint {url:?}")))?;

    if re_sdk::forced_sink_path().is_some() {
        re_log::debug!("Ignored call to `connect_grpc()` since _RERUN_TEST_FORCE_SAVE is set");
        return Ok(());
    }

    py.allow_threads(|| {
        let sink = re_sdk::sink::GrpcSink::new(uri);

        if let Some(default_blueprint) = default_blueprint {
            send_mem_sink_as_default_blueprint(&sink, default_blueprint);
        }

        recording.set_sink(Box::new(sink));

        flush_garbage_queue();
    });

    Ok(())
}

#[pyfunction]
#[pyo3(signature = (url, make_active, make_default, blueprint_stream))]
/// Special binding for directly sending a blueprint stream to a connection.
fn connect_grpc_blueprint(
    url: Option<String>,
    make_active: bool,
    make_default: bool,
    blueprint_stream: &PyRecordingStream,
    py: Python<'_>,
) -> PyResult<()> {
    let url = url.unwrap_or_else(|| re_sdk::DEFAULT_CONNECT_URL.to_owned());

    if let Some(blueprint_id) = blueprint_stream.store_info().map(|info| info.store_id) {
        // The call to save, needs to flush.
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| -> PyResult<()> {
            // Flush all the pending blueprint messages before we include the Ready message
            blueprint_stream
                .flush_blocking()
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let activation_cmd = BlueprintActivationCommand {
                blueprint_id,
                make_active,
                make_default,
            };

            blueprint_stream.record_msg(activation_cmd.into());

            blueprint_stream
                .connect_grpc_opts(url)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            flush_garbage_queue();
            Ok(())
        })
    } else {
        Err(PyRuntimeError::new_err(
            "Blueprint stream has no store info".to_owned(),
        ))
    }
}

/// Save the recording stream to a file.
#[pyfunction]
#[pyo3(signature = (path, default_blueprint = None, recording = None))]
fn save(
    path: &str,
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    if re_sdk::forced_sink_path().is_some() {
        re_log::debug!("Ignored call to `save()` since _RERUN_TEST_FORCE_SAVE is set");
        return Ok(());
    }

    // The call to save may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        // We create the sink manually so we can send the default blueprint
        // first before the rest of the current recording stream.
        let sink = re_sdk::sink::FileSink::new(path)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        if let Some(default_blueprint) = default_blueprint {
            send_mem_sink_as_default_blueprint(&sink, default_blueprint);
        }

        recording.set_sink(Box::new(sink));

        flush_garbage_queue();

        Ok(())
    })
}

#[pyfunction]
#[pyo3(signature = (path, blueprint_stream))]
/// Special binding for directly savings a blueprint stream to a file.
fn save_blueprint(
    path: &str,
    blueprint_stream: &PyRecordingStream,
    py: Python<'_>,
) -> PyResult<()> {
    if let Some(blueprint_id) = (*blueprint_stream).store_info().map(|info| info.store_id) {
        // The call to save, needs to flush.
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| -> PyResult<_> {
            // Flush all the pending blueprint messages before we include the Ready message
            blueprint_stream
                .flush_blocking()
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let activation_cmd = BlueprintActivationCommand::make_active(blueprint_id.clone());

            blueprint_stream.record_msg(activation_cmd.into());

            let res = blueprint_stream
                .save_opts(path)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()));
            flush_garbage_queue();
            res
        })
    } else {
        Err(PyRuntimeError::new_err(
            "Blueprint stream has no store info".to_owned(),
        ))
    }
}

/// Save to stdout.
#[pyfunction]
#[pyo3(signature = (default_blueprint = None, recording = None))]
fn stdout(
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    if re_sdk::forced_sink_path().is_some() {
        re_log::debug!("Ignored call to `stdout()` since _RERUN_TEST_FORCE_SAVE is set");
        return Ok(());
    }

    // The call to stdout may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        let sink: Box<dyn re_sdk::sink::LogSink> = if std::io::stdout().is_terminal() {
            re_log::debug!("Ignored call to stdout() because stdout is a terminal");
            Box::new(re_sdk::sink::BufferedSink::new())
        } else {
            Box::new(
                re_sdk::sink::FileSink::stdout()
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
            )
        };

        if let Some(default_blueprint) = default_blueprint {
            send_mem_sink_as_default_blueprint(sink.as_ref(), default_blueprint);
        }

        flush_garbage_queue();

        recording.set_sink(sink);

        Ok(())
    })
}

/// Create an in-memory rrd file.
#[pyfunction]
#[pyo3(signature = (recording = None))]
fn memory_recording(
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> Option<PyMemorySinkStorage> {
    get_data_recording(recording).map(|rec| {
        // The call to memory may internally flush.
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        let inner = py.allow_threads(|| {
            let storage = rec.memory();
            flush_garbage_queue();
            storage
        });
        PyMemorySinkStorage { inner }
    })
}

/// Set callback sink.
#[pyfunction]
#[pyo3(signature = (callback, recording = None))]
fn set_callback_sink(callback: PyObject, recording: Option<&PyRecordingStream>, py: Python<'_>) {
    let Some(rec) = get_data_recording(recording) else {
        return;
    };

    let callback = move |msgs: &[LogMsg]| {
        Python::with_gil(|py| {
            let data = Encoder::encode(msgs.iter().map(Ok)).ok_or_log_error()?;
            let bytes = PyBytes::new(py, &data);
            callback.bind(py).call1((bytes,)).ok_or_log_error()?;
            Some(())
        });
    };

    // The call to `set_sink` may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        rec.set_sink(Box::new(CallbackSink::new(callback)));
        flush_garbage_queue();
    });
}

/// Set callback sink for blueprint.
#[pyfunction]
#[pyo3(signature = (callback, make_active, make_default, blueprint_stream))]
fn set_callback_sink_blueprint(
    callback: PyObject,
    make_active: bool,
    make_default: bool,
    blueprint_stream: &PyRecordingStream,
    py: Python<'_>,
) -> PyResult<()> {
    let Some(blueprint_id) = (*blueprint_stream).store_info().map(|info| info.store_id) else {
        return Ok(());
    };

    let callback = move |msgs: &[LogMsg]| {
        Python::with_gil(|py| {
            let data = Encoder::encode(msgs.iter().map(Ok)).ok_or_log_error()?;
            let bytes = PyBytes::new(py, &data);
            callback.bind(py).call1((bytes,)).ok_or_log_error()?;
            Some(())
        });
    };

    // The call to `set_sink` may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| -> PyResult<()> {
        blueprint_stream
            .flush_blocking()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        let activation_cmd = BlueprintActivationCommand {
            blueprint_id,
            make_active,
            make_default,
        };

        blueprint_stream.record_msg(activation_cmd.into());

        blueprint_stream.set_sink(Box::new(CallbackSink::new(callback)));
        flush_garbage_queue();
        Ok(())
    })
}

/// Create a new binary stream sink, and return the associated binary stream.
#[pyfunction]
#[pyo3(signature = (recording = None))]
fn binary_stream(
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> Option<PyBinarySinkStorage> {
    let recording = get_data_recording(recording)?;

    // The call to memory may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    let inner = py.allow_threads(|| {
        let storage = recording.binary_stream();
        flush_garbage_queue();
        storage
    });

    Some(PyBinarySinkStorage { inner })
}

#[pyclass(frozen, module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
struct PyMemorySinkStorage {
    // So we can flush when needed!
    inner: MemorySinkStorage,
}

#[pymethods]
impl PyMemorySinkStorage {
    /// Concatenate the contents of the [`MemorySinkStorage`] as bytes.
    ///
    /// Note: This will do a blocking flush before returning!
    #[pyo3(signature = (concat=None))]
    fn concat_as_bytes<'p>(
        &self,
        concat: Option<&Self>,
        py: Python<'p>,
    ) -> PyResult<Bound<'p, PyBytes>> {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            let concat_bytes = MemorySinkStorage::concat_memory_sinks_as_bytes(
                [Some(&self.inner), concat.map(|c| &c.inner)]
                    .iter()
                    .filter_map(|s| *s)
                    .collect_vec()
                    .as_slice(),
            );

            flush_garbage_queue();

            concat_bytes
        })
        .map(|bytes| PyBytes::new(py, bytes.as_slice()))
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }

    /// Count the number of pending messages in the [`MemorySinkStorage`].
    ///
    /// This will do a blocking flush before returning!
    fn num_msgs(&self, py: Python<'_>) -> usize {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            let num = self.inner.num_msgs();

            flush_garbage_queue();

            num
        })
    }

    /// Drain all messages logged to the [`MemorySinkStorage`] and return as bytes.
    ///
    /// This will do a blocking flush before returning!
    fn drain_as_bytes<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyBytes>> {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            let bytes = self.inner.drain_as_bytes();

            flush_garbage_queue();

            bytes
        })
        .map(|bytes| PyBytes::new(py, bytes.as_slice()))
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }

    pub fn __repr__(&self) -> String {
        format!("MemorySinkStorage({:#?})", self.inner.store_id())
    }
}

#[pyclass(frozen, module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
struct PyBinarySinkStorage {
    /// The underlying binary sink storage.
    inner: BinaryStreamStorage,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyBinarySinkStorage {
    /// Read the bytes from the binary sink.
    ///
    /// If `flush` is `True`, the sink will be flushed before reading.
    /// If all the data was not successfully flushed within the given timeout,
    /// an exception will be raised.
    ///
    /// Parameters
    /// ----------
    /// flush:
    ///     If true (default), the stream will be flushed before reading.
    /// flush_timeout_sec:
    ///     If `flush` is `True`, wait at most this many seconds.
    ///     If the timeout is reached, an error is raised.
    #[pyo3(signature = (*, flush = true, flush_timeout_sec = 1e38))] // Can't use infinity here because of python_check_signatures.py
    fn read<'p>(
        &self,
        py: Python<'p>,
        flush: bool,
        flush_timeout_sec: f32,
    ) -> PyResult<Option<Bound<'p, PyBytes>>> {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| -> PyResult<_> {
            if flush {
                let timeout = duration_from_sec(flush_timeout_sec as _)?;
                self.inner
                    .flush(timeout)
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            }

            let bytes = self.inner.read();

            flush_garbage_queue();

            Ok(bytes)
        })
        .map(|bytes| bytes.map(|bytes| PyBytes::new(py, &bytes)))
    }

    /// Flushes the binary sink and ensures that all logged messages have been encoded into the stream.
    ///
    /// This will block until the flush is complete, or the timeout is reached, or an error occurs.
    ///
    /// If all the data was not successfully flushed within the given timeout,
    /// an exception will be raised.
    ///
    /// Parameters
    /// ----------
    /// timeout_sec:
    ///     Wait at most this many seconds.
    ///     If the timeout is reached, an error is raised.
    #[pyo3(signature = (*, timeout_sec = 1e38))] // Can't use infinity here because of python_check_signatures.py
    fn flush(&self, py: Python<'_>, timeout_sec: f32) -> PyResult<()> {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| -> PyResult<_> {
            let timeout = duration_from_sec(timeout_sec as _)?;
            self.inner
                .flush(timeout)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            flush_garbage_queue();
            Ok(())
        })
    }
}

fn duration_from_sec(seconds: f64) -> PyResult<Duration> {
    if seconds.is_nan() {
        Err(PyRuntimeError::new_err("duration must not be NaN"))
    } else if seconds < 0.0 {
        Err(PyRuntimeError::new_err("duration must be non-negative"))
    } else {
        Ok(Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX))
    }
}

/// Spawn a gRPC server which an SDK or Viewer can connect to.
///
/// Returns the URI of the server so you can connect the viewer to it.
#[pyfunction]
#[pyo3(signature = (grpc_port, server_memory_limit, newest_first = false, default_blueprint = None, recording = None))]
fn serve_grpc(
    grpc_port: Option<u16>,
    server_memory_limit: String,
    newest_first: bool,
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
) -> PyResult<String> {
    #[cfg(feature = "server")]
    {
        let Some(recording) = get_data_recording(recording) else {
            return Ok("[no active recording]".to_owned());
        };

        if re_sdk::forced_sink_path().is_some() {
            re_log::debug!("Ignored call to `serve_grpc()` since _RERUN_TEST_FORCE_SAVE is set");
            return Ok("[_RERUN_TEST_FORCE_SAVE is set]".to_owned());
        }

        let server_options = re_sdk::ServerOptions {
            playback_behavior: re_sdk::PlaybackBehavior::from_newest_first(newest_first),

            memory_limit: re_memory::MemoryLimit::parse(&server_memory_limit).map_err(|err| {
                PyRuntimeError::new_err(format!("Bad server_memory_limit: {err}:"))
            })?,
        };

        let sink = re_sdk::grpc_server::GrpcServerSink::new(
            "0.0.0.0",
            grpc_port.unwrap_or(re_grpc_server::DEFAULT_SERVER_PORT),
            server_options,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        if let Some(default_blueprint) = default_blueprint {
            send_mem_sink_as_default_blueprint(&sink, default_blueprint);
        }

        let uri = sink.uri().to_string();

        recording.set_sink(Box::new(sink));

        Ok(uri)
    }

    #[cfg(not(feature = "server"))]
    {
        let _ = (
            grpc_port,
            server_memory_limit,
            newest_first,
            default_blueprint,
            recording,
        );

        Err(PyRuntimeError::new_err(
            "The Rerun SDK was not compiled with the 'server' feature",
        ))
    }
}

/// Serve a web-viewer over HTTP.
///
/// This only serves HTML+JS+Wasm, but does NOT host a gRPC server.
#[allow(clippy::allow_attributes, clippy::unnecessary_wraps)] // False positive
#[pyfunction]
#[pyo3(signature = (web_port = None, open_browser = true, connect_to = None))]
fn serve_web_viewer(
    web_port: Option<u16>,
    open_browser: bool,
    connect_to: Option<String>,
) -> PyResult<()> {
    #[cfg(feature = "web_viewer")]
    {
        re_sdk::web_viewer::WebViewerConfig {
            open_browser,
            connect_to: connect_to.into_iter().collect(),
            web_port: web_port.map(WebViewerServerPort).unwrap_or_default(),
            ..Default::default()
        }
        .host_web_viewer()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
        .detach();

        Ok(())
    }

    #[cfg(not(feature = "web_viewer"))]
    {
        _ = web_port;
        _ = open_browser;
        _ = connect_to;
        Err(PyRuntimeError::new_err(
            "The Rerun SDK was not compiled with the 'web_viewer' feature",
        ))
    }
}

/// Serve a web-viewer AND host a gRPC server.
// NOTE: DEPRECATED
#[allow(clippy::allow_attributes, clippy::unnecessary_wraps)] // False positive
#[pyfunction]
#[pyo3(signature = (open_browser, web_port, grpc_port, server_memory_limit, default_blueprint = None, recording = None))]
fn serve_web(
    open_browser: bool,
    web_port: Option<u16>,
    grpc_port: Option<u16>,
    server_memory_limit: String,
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    #[cfg(feature = "web_viewer")]
    {
        let Some(recording) = get_data_recording(recording) else {
            return Ok(());
        };

        if re_sdk::forced_sink_path().is_some() {
            re_log::debug!("Ignored call to `serve()` since _RERUN_TEST_FORCE_SAVE is set");
            return Ok(());
        }

        let server_options = re_sdk::ServerOptions {
            memory_limit: re_memory::MemoryLimit::parse(&server_memory_limit).map_err(|err| {
                PyRuntimeError::new_err(format!("Bad server_memory_limit: {err}:"))
            })?,
            playback_behavior: re_grpc_server::PlaybackBehavior::OldestFirst,
        };

        let sink = re_sdk::web_viewer::new_sink(
            open_browser,
            "0.0.0.0",
            web_port.map(WebViewerServerPort).unwrap_or_default(),
            grpc_port.unwrap_or(re_grpc_server::DEFAULT_SERVER_PORT),
            server_options,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        if let Some(default_blueprint) = default_blueprint {
            send_mem_sink_as_default_blueprint(sink.as_ref(), default_blueprint);
        }

        recording.set_sink(sink);

        Ok(())
    }

    #[cfg(not(feature = "web_viewer"))]
    {
        _ = default_blueprint;
        _ = recording;
        _ = web_port;
        _ = grpc_port;
        _ = open_browser;
        _ = server_memory_limit;
        Err(PyRuntimeError::new_err(
            "The Rerun SDK was not compiled with the 'web_viewer' feature",
        ))
    }
}

/// Disconnect from remote server (if any).
///
/// Subsequent log messages will be buffered and either sent on the next call to `connect_grpc` or `spawn`.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn disconnect(py: Python<'_>, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        recording.disconnect();
        flush_garbage_queue();
    });
}

/// Block until outstanding data has been flushed to the sink.
#[pyfunction]
#[pyo3(signature = (*, timeout_sec = 1e38, recording = None))] // Can't use infinity here because of python_check_signatures.py
fn flush(py: Python<'_>, timeout_sec: f32, recording: Option<&PyRecordingStream>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| -> PyResult<()> {
        if timeout_sec == 0.0 {
            recording
                .flush_async()
                .map_err(|err: SinkFlushError| PyRuntimeError::new_err(err.to_string()))?;
        } else {
            recording
                .flush_with_timeout(duration_from_sec(timeout_sec as _)?)
                .map_err(|err: SinkFlushError| PyRuntimeError::new_err(err.to_string()))?;
        }
        flush_garbage_queue();
        Ok(())
    })
}

// --- Components ---

/// A `ComponentDescriptor` fully describes the semantics of a column of data.
///
/// Every component at a given entity path is uniquely identified by the
/// `component` field of the descriptor. The `archetype` and `component_type`
/// fields provide additional information about the semantics of the data.
#[pyclass(
    eq,
    name = "ComponentDescriptor",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyComponentDescriptor(pub ComponentDescriptor);

#[pymethods]
impl PyComponentDescriptor {
    /// Creates a component descriptor.
    #[new]
    #[pyo3(signature = (component, archetype=None, component_type=None))]
    #[pyo3(text_signature = "(self, component, archetype=None, component_type=None)")]
    fn new(component: &str, archetype: Option<&str>, component_type: Option<&str>) -> Self {
        let descr = ComponentDescriptor {
            archetype: archetype.map(Into::into),
            component: component.into(),
            component_type: component_type.map(Into::into),
        };

        Self(descr)
    }

    fn __hash__(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash as _, Hasher as _};

        let mut hasher = DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }

    fn __str__(&self) -> String {
        self.0.display_name().to_owned()
    }

    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    #[getter]
    fn archetype(&self) -> Option<String> {
        self.0.archetype.map(|a| a.to_string())
    }

    /// Uniquely identifies of the component associated with this data.
    ///
    /// Example: `Points3D:positions`.
    #[getter]
    fn component(&self) -> String {
        self.0.component.to_string()
    }

    /// Optional type information for this component.
    ///
    /// Can be used to inform applications on how to interpret the data.
    ///
    /// Example: `rerun.components.Position3D`.
    #[getter]
    fn component_type(&self) -> Option<String> {
        self.0.component_type.map(|a| a.to_string())
    }

    /// Unconditionally sets `archetype` and `component_type` to the given ones (if specified).
    #[pyo3(signature = (archetype=None, component_type=None))]
    fn with_overrides(&mut self, archetype: Option<&str>, component_type: Option<&str>) -> Self {
        let mut cloned = self.0.clone();
        if let Some(archetype) = archetype {
            cloned = cloned.with_archetype(archetype.into());
        }
        if let Some(component_type) = component_type {
            cloned = cloned.with_component_type(component_type.into());
        }
        Self(cloned)
    }

    /// Sets `archetype` and `component_type` to the given one iff it's not already set.
    #[pyo3(signature = (archetype=None, component_type=None))]
    fn or_with_overrides(&mut self, archetype: Option<&str>, component_type: Option<&str>) -> Self {
        let mut cloned = self.0.clone();
        if let Some(archetype) = archetype {
            cloned = cloned.or_with_archetype(|| archetype.into());
        }
        if let Some(component_type) = component_type {
            cloned = cloned.or_with_component_type(|| component_type.into());
        }
        Self(cloned)
    }

    /// Sets `archetype` in a format similar to built-in archetypes.
    fn with_builtin_archetype(&mut self, archetype: &str) -> Self {
        Self(self.0.clone().with_builtin_archetype(archetype))
    }
}

// --- Time ---

/// Set the current time for this thread as an integer sequence.
#[pyfunction]
#[pyo3(signature = (timeline, sequence, recording=None))]
fn set_time_sequence(timeline: &str, sequence: i64, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time(timeline, TimeCell::from_sequence(sequence));
}

/// Set the current duration for this thread in nanoseconds.
#[pyfunction]
#[pyo3(signature = (timeline, nanos, recording=None))]
fn set_time_duration_nanos(timeline: &str, nanos: i64, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time(timeline, TimeCell::from_duration_nanos(nanos));
}

/// Set the current time for this thread in nanoseconds.
#[pyfunction]
#[pyo3(signature = (timeline, nanos, recording=None))]
fn set_time_timestamp_nanos_since_epoch(
    timeline: &str,
    nanos: i64,
    recording: Option<&PyRecordingStream>,
) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time(timeline, TimeCell::from_timestamp_nanos_since_epoch(nanos));
}

/// Clear time information for the specified timeline on this thread.
#[pyfunction]
#[pyo3(signature = (timeline, recording=None))]
fn disable_timeline(timeline: &str, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.disable_timeline(timeline);
}

/// Clear all timeline information on this thread.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn reset_time(recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.reset_time();
}

// --- Log special ---

/// Log an arrow message.
#[pyfunction]
#[pyo3(signature = (
    entity_path,
    components,
    static_,
    recording=None,
))]
fn log_arrow_msg(
    py: Python<'_>,
    entity_path: &str,
    components: Bound<'_, PyDict>,
    static_: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let entity_path = EntityPath::parse_forgiving(entity_path);

    let row = crate::arrow::build_row_from_components(&components, &TimePoint::default())?;

    py.allow_threads(|| {
        // The call to `allow_threads` releases the GIL.
        // It is important that we do so here,
        // because the destructor for the arrow data will acquire the GIL
        // in order to call back to pyarrow, and that can cause a deadlock,
        // and the call here may cause that destructor to be invoked.
        recording.record_row(entity_path, row, !static_);

        flush_garbage_queue();

        Ok(())
    })
}

/// Directly send an arrow chunk to the recording stream.
///
/// Params
/// ------
/// entity_path: `str`
///     The entity path to log the chunk to.
/// timelines: `Dict[str, arrow::Int64Array]`
///     A dictionary mapping timeline names to their values.
/// components: `Dict[ComponentDescriptor, arrow::ListArray]`
///     A dictionary mapping component types to their values.
#[pyfunction]
#[pyo3(signature = (
    entity_path,
    timelines,
    components,
    recording=None,
))]
fn send_arrow_chunk(
    py: Python<'_>,
    entity_path: &str,
    timelines: Bound<'_, PyDict>,
    components: Bound<'_, PyDict>,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let entity_path = EntityPath::parse_forgiving(entity_path);

    // It's important that we don't hold the session lock while building our arrow component.
    // the API we call to back through pyarrow temporarily releases the GIL, which can cause
    // a deadlock.
    let chunk = crate::arrow::build_chunk_from_components(entity_path, &timelines, &components)?;

    py.allow_threads(|| {
        // The call to `allow_threads` releases the GIL.
        // It is important that we do so here,
        // because the destructor for the arrow data will acquire the GIL
        // in order to call back to pyarrow, and that can cause a deadlock,
        // and the call here may cause that destructor to be invoked.
        recording.send_chunk(chunk);

        flush_garbage_queue();

        Ok(())
    })
}

/// Log a file by path.
#[pyfunction]
#[pyo3(signature = (
    file_path,
    entity_path_prefix = None,
    static_ = false,
    recording = None,
))]
fn log_file_from_path(
    py: Python<'_>,
    file_path: std::path::PathBuf,
    entity_path_prefix: Option<String>,
    static_: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    log_file(py, file_path, None, entity_path_prefix, static_, recording)
}

/// Log a file by contents.
#[pyfunction]
#[pyo3(signature = (
    file_path,
    file_contents,
    entity_path_prefix = None,
    static_ = false,
    recording = None,
))]
fn log_file_from_contents(
    py: Python<'_>,
    file_path: std::path::PathBuf,
    file_contents: &[u8],
    entity_path_prefix: Option<String>,
    static_: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    log_file(
        py,
        file_path,
        Some(file_contents),
        entity_path_prefix,
        static_,
        recording,
    )
}

fn log_file(
    py: Python<'_>,
    file_path: std::path::PathBuf,
    file_contents: Option<&[u8]>,
    entity_path_prefix: Option<String>,
    static_: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    if let Some(contents) = file_contents {
        recording
            .log_file_from_contents(
                file_path,
                std::borrow::Cow::Borrowed(contents),
                entity_path_prefix.map(Into::into),
                static_,
            )
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    } else {
        recording
            .log_file_from_path(file_path, entity_path_prefix.map(Into::into), static_)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    }

    py.allow_threads(flush_garbage_queue);

    Ok(())
}

/// Send a blueprint to the given recording stream.
#[pyfunction]
#[pyo3(signature = (blueprint, make_active = false, make_default = true, recording = None))]
fn send_blueprint(
    blueprint: &PyMemorySinkStorage,
    make_active: bool,
    make_default: bool,
    recording: Option<&PyRecordingStream>,
) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };

    if let Some(blueprint_id) = blueprint.inner.store_id() {
        let activation_cmd = BlueprintActivationCommand {
            blueprint_id,
            make_active,
            make_default,
        };

        recording.send_blueprint(blueprint.inner.take(), activation_cmd);
    } else {
        re_log::warn!("Provided `blueprint` has no store info, cannot send it.");
    }
}

/// Send all chunks from a [`PyRecording`] to the given recording stream.
///
/// .. warning::
///     ⚠️ This API is experimental and may change or be removed in future versions! ⚠️
#[pyfunction]
#[pyo3(signature = (rrd, recording = None))]
fn send_recording(rrd: &PyRecording, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };

    let store = rrd.store.read();
    for chunk in store.iter_physical_chunks() {
        recording.send_chunk((**chunk).clone());
    }
}

// --- Misc ---

/// Return a verbose version string.
#[pyfunction]
fn version() -> String {
    re_build_info::build_info!().to_string()
}

/// Return True if the Rerun SDK is a dev/debug build.
#[pyfunction]
fn is_dev_build() -> bool {
    cfg!(debug_assertions)
}

/// Get an url to an instance of the web-viewer.
///
/// This may point to app.rerun.io or localhost depending on
/// whether [`start_web_viewer_server()`] was called.
#[pyfunction]
fn get_app_url() -> String {
    #[cfg(feature = "web_viewer")]
    if let Some(hosted_assets) = &*global_web_viewer_server() {
        return hosted_assets.server_url();
    }

    let build_info = re_build_info::build_info!();

    // Note that it is important to us `app.rerun.io` directly here. The version hosted
    // at `rerun.io/viewer` is not designed to be embedded in a notebook and interferes
    // with the startup sequencing. Do not switch to `rerun.io/viewer` without considering
    // the implications.
    if build_info.is_final() {
        format!("https://app.rerun.io/version/{}", build_info.version)
    } else if let Some(short_git_hash) = build_info.git_hash.get(..7) {
        format!("https://app.rerun.io/commit/{short_git_hash}")
    } else {
        re_log::warn_once!(
            "No valid git hash found in build info. Defaulting to app.rerun.io for app url."
        );
        "https://app.rerun.io".to_owned()
    }
}

// TODO(jleibs) expose this as a python type
/// Start a web server to host the run web-assets.
#[allow(clippy::allow_attributes, clippy::unnecessary_wraps)] // false positive
#[pyfunction]
fn start_web_viewer_server(port: u16) -> PyResult<()> {
    #[cfg(feature = "web_viewer")]
    {
        let mut web_handle = global_web_viewer_server();

        *web_handle = Some(
            re_web_viewer_server::WebViewerServer::new("0.0.0.0", WebViewerServerPort(port))
                .map_err(|err| {
                    PyRuntimeError::new_err(format!(
                        "Failed to start web viewer server on port {port}: {err}",
                    ))
                })?,
        );

        Ok(())
    }

    #[cfg(not(feature = "web_viewer"))]
    {
        _ = port;
        Err(PyRuntimeError::new_err(
            "The Rerun SDK was not compiled with the 'web_viewer' feature",
        ))
    }
}

/// Escape an entity path.
#[pyfunction]
fn escape_entity_path_part(part: &str) -> String {
    EntityPathPart::from(part).escaped_string()
}

/// Create an entity path.
#[pyfunction]
fn new_entity_path(parts: Vec<Bound<'_, pyo3::types::PyString>>) -> PyResult<String> {
    let parts: PyResult<Vec<_>> = parts.iter().map(|part| part.to_cow()).collect();
    let path = EntityPath::from(
        parts?
            .into_iter()
            .map(|part| EntityPathPart::from(part.borrow()))
            .collect_vec(),
    );
    Ok(path.to_string())
}

// --- Properties ---

/// Create a property entity path.
#[pyfunction]
fn new_property_entity_path(parts: Vec<Bound<'_, pyo3::types::PyString>>) -> PyResult<String> {
    let parts: PyResult<Vec<_>> = parts.iter().map(|part| part.to_cow()).collect();
    let path = EntityPath::from(
        parts?
            .into_iter()
            .map(|part| EntityPathPart::from(part.borrow()))
            .collect_vec(),
    );
    Ok(EntityPath::properties().join(&path).to_string())
}

/// Send the name of the recording.
#[pyfunction]
#[pyo3(signature = (name, recording=None))]
fn send_recording_name(name: &str, recording: Option<&PyRecordingStream>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };
    recording
        .send_recording_name(name)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

/// Send the start time of the recording.
#[pyfunction]
#[pyo3(signature = (nanos, recording=None))]
fn send_recording_start_time_nanos(
    nanos: i64,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };
    recording
        .send_recording_start_time(nanos)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

// --- Helpers ---

pub fn python_version(py: Python<'_>) -> re_log_types::PythonVersion {
    let py_version = py.version_info();
    re_log_types::PythonVersion {
        major: py_version.major,
        minor: py_version.minor,
        patch: py_version.patch,
        suffix: py_version.suffix.map(|s| s.to_owned()).unwrap_or_default(),
    }
}

fn default_recording_id(py: Python<'_>, application_id: &str) -> RecordingId {
    use std::hash::{Hash as _, Hasher as _};

    use rand::{Rng as _, SeedableRng as _};

    // If the user uses `multiprocessing` for parallelism,
    // we still want child processes to log to the same recording.
    // We can use authkey for this, because it is the same for parent
    // and child processes.
    //
    // TODO(emilk): are there any security concerns with leaking authkey?
    //
    // https://docs.python.org/3/library/multiprocessing.html#multiprocessing.Process.authkey
    let seed = match authkey(py) {
        Ok(seed) => seed,
        Err(err) => {
            re_log::error_once!(
                "Failed to retrieve python authkey: {err}\nMultiprocessing will result in split recordings."
            );
            // If authkey failed, just generate a random 8-byte authkey
            let bytes = rand::Rng::random::<[u8; 8]>(&mut rand::rng());
            bytes.to_vec()
        }
    };
    let salt: u64 = 0xab12_cd34_ef56_0178;

    let mut hasher = std::collections::hash_map::DefaultHasher::default();
    seed.hash(&mut hasher);
    salt.hash(&mut hasher);
    // NOTE: We hash the application ID too!
    //
    // This makes sure that independent recording streams started from the same program, but
    // targeting different application IDs, won't share the same recording ID.
    application_id.hash(&mut hasher);
    let mut rng = rand::rngs::StdRng::seed_from_u64(hasher.finish());
    let uuid = uuid::Builder::from_random_bytes(rng.random()).into_uuid();
    RecordingId::from(uuid.simple().to_string())
}

fn authkey(py: Python<'_>) -> PyResult<Vec<u8>> {
    let locals = PyDict::new(py);

    py.run(
        cr#"
import multiprocessing
# authkey is the same for child and parent processes, so this is how we know we're the same
authkey = multiprocessing.current_process().authkey
            "#,
        None,
        Some(&locals),
    )
    .and_then(|()| {
        locals
            .get_item("authkey")?
            .ok_or_else(|| PyRuntimeError::new_err("authkey missing from expected locals"))
    })
    .and_then(|authkey| {
        authkey
            .downcast()
            .cloned()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    })
    .map(|authkey: Bound<'_, PyBytes>| authkey.as_bytes().to_vec())
}

#[pyclass(name = "DeviceCodeFlow", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
struct PyDeviceCodeFlow {
    login_flow: DeviceCodeFlow,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyDeviceCodeFlow {
    /// Get the URL for the OAuth login flow.
    fn login_url(&self) -> String {
        self.login_flow.get_login_url().to_owned()
    }

    /// Get the user code.
    fn user_code(&self) -> String {
        self.login_flow.get_user_code().to_owned()
    }

    /// Finish the OAuth login flow.
    ///
    /// Returns
    /// -------
    /// Credentials
    ///     The credentials of the logged in user.
    fn finish_login_flow(&mut self, py: Python<'_>) -> PyResult<PyCredentials> {
        crate::utils::wait_for_future(py, async move {
            tokio::select! {
                result = self.login_flow.wait_for_user_confirmation() => {
                    result
                        .map(PyCredentials)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
                }
                _ = tokio::signal::ctrl_c() => {
                    Err(PyKeyboardInterrupt::new_err(None::<()>))
                }
            }
        })
    }
}

#[pyfunction]
/// Initialize an OAuth login flow.
///
/// Returns
/// -------
/// DeviceCodeFlow | None
///     The login flow, or `None` if the user is already logged in.
fn init_login_flow(py: Python<'_>) -> PyResult<Option<PyDeviceCodeFlow>> {
    let login_flow = crate::utils::wait_for_future(py, async { DeviceCodeFlow::init(false).await })
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    match login_flow {
        DeviceCodeFlowState::AlreadyLoggedIn(_) => {
            // Already logged in, no need to start a login flow.
            Ok(None)
        }
        DeviceCodeFlowState::LoginFlowStarted(login_flow) => {
            Ok(Some(PyDeviceCodeFlow { login_flow }))
        }
    }
}

#[pyclass(
    frozen,
    eq,
    name = "Credentials",
    module = "rerun_bindings.rerun_bindings"
)]
/// The credentials for the OAuth login flow.
struct PyCredentials(Credentials);

#[pymethods]
impl PyCredentials {
    #[getter]
    /// The access token.
    fn access_token(&self) -> String {
        self.0.access_token().as_str().to_owned()
    }

    #[getter]
    /// The user email.
    fn user_email(&self) -> String {
        self.0.user().email.clone()
    }

    pub fn __str__(&self) -> String {
        format!("<Credentials for '{}'>", self.user_email())
    }
}

impl std::cmp::PartialEq for PyCredentials {
    fn eq(&self, other: &Self) -> bool {
        self.access_token() == other.access_token()
    }
}

#[pyfunction]
/// Returns the credentials for the current user.
fn get_credentials(py: Python<'_>) -> PyResult<Option<PyCredentials>> {
    let Some(credentials) = re_auth::oauth::load_credentials()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
    else {
        // No credentials found.
        return Ok(None);
    };

    let credentials = crate::utils::wait_for_future(py, async {
        re_auth::oauth::refresh_credentials(credentials).await
    })
    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    Ok(Some(PyCredentials(credentials)))
}

#[pyfunction]
/// Check if the RRD has a valid RRD footer.
///
/// This is useful for unit-tests to verify that data has been fully flushed to disk.
fn check_for_rrd_footer(file_path: std::path::PathBuf) -> PyResult<bool> {
    let rrd_bytes =
        std::fs::read(file_path).map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    let rrd_manifests = re_log_encoding::RawRrdManifest::from_rrd_bytes(&rrd_bytes)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    Ok(!rrd_manifests.is_empty())
}
