#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use std::collections::HashMap;
use std::path::PathBuf;

use itertools::Itertools;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyDict},
};

use re_viewport::VIEWPORT_PATH;

use re_log_types::{DataRow, EntityPathPart, StoreKind};
use rerun::{
    log::RowId, sink::MemorySinkStorage, time::TimePoint, EntityPath, RecordingStream,
    RecordingStreamBuilder, StoreId,
};

#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "web_viewer")]
use re_ws_comms::RerunServerPort;

// --- FFI ---

use once_cell::sync::{Lazy, OnceCell};

// The bridge needs to have complete control over the lifetimes of the individual recordings,
// otherwise all the recording shutdown machinery (which includes deallocating C, Rust and Python
// data and joining a bunch of threads) can end up running at any time depending on what the
// Python GC is doing, which obviously leads to very bad things :tm:.
//
// TODO(#2116): drop unused recordings
fn all_recordings() -> parking_lot::MutexGuard<'static, HashMap<StoreId, RecordingStream>> {
    static ALL_RECORDINGS: OnceCell<parking_lot::Mutex<HashMap<StoreId, RecordingStream>>> =
        OnceCell::new();
    ALL_RECORDINGS.get_or_init(Default::default).lock()
}

type GarbageChunk = arrow2::chunk::Chunk<Box<dyn arrow2::array::Array>>;
type GarbageSender = crossbeam::channel::Sender<GarbageChunk>;
type GarbageReceiver = crossbeam::channel::Receiver<GarbageChunk>;

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
static GARBAGE_QUEUE: Lazy<(GarbageSender, GarbageReceiver)> =
    Lazy::new(crossbeam::channel::unbounded);

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
fn global_web_viewer_server(
) -> parking_lot::MutexGuard<'static, Option<re_web_viewer_server::WebViewerServerHandle>> {
    static WEB_HANDLE: OnceCell<
        parking_lot::Mutex<Option<re_web_viewer_server::WebViewerServerHandle>>,
    > = OnceCell::new();
    WEB_HANDLE.get_or_init(Default::default).lock()
}

#[pyfunction]
fn main(py: Python<'_>) -> PyResult<u8> {
    // We access argv ourselves instead of accepting as parameter, so that `main`'s signature is
    // compatible with `[project.scripts]` in `pyproject.toml`.
    let sys = py.import("sys")?;
    let argv: Vec<String> = sys.getattr("argv")?.extract()?;

    let build_info = re_build_info::build_info!();
    let call_src = rerun::CallSource::Python(python_version(py));
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // Python catches SIGINT and waits for us to release the GIL before shutting down.
            // That's no good, so we need to catch SIGINT ourselves and shut down:
            tokio::spawn(async move {
                tokio::signal::ctrl_c().await.unwrap();
                eprintln!("Ctrl-C detected in rerun_py. Shutting down.");
                #[allow(clippy::exit)]
                std::process::exit(42);
            });

            rerun::run(build_info, call_src, argv).await
        })
        .map_err(|err| PyRuntimeError::new_err(re_error::format(err)))
}

/// The python module is called "rerun_bindings".
#[pymodule]
fn rerun_bindings(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // NOTE: We do this here because some the inner init methods don't respond too kindly to being
    // called more than once.
    re_log::setup_logging();

    // We always want main to be available
    m.add_function(wrap_pyfunction!(main, m)?)?;

    // These two components are necessary for imports to work
    m.add_class::<PyMemorySinkStorage>()?;
    m.add_class::<PyRecordingStream>()?;

    // If this is a special RERUN_APP_ONLY context (launched via .spawn), we
    // can bypass everything else, which keeps us from preparing an SDK session
    // that never gets used.
    if matches!(std::env::var("RERUN_APP_ONLY").as_deref(), Ok("true")) {
        return Ok(());
    }

    // init
    m.add_function(wrap_pyfunction!(new_recording, m)?)?;
    m.add_function(wrap_pyfunction!(new_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;
    m.add_function(wrap_pyfunction!(cleanup_if_forked_child, m)?)?;

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

    // sinks
    m.add_function(wrap_pyfunction!(is_enabled, m)?)?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add_function(wrap_pyfunction!(save, m)?)?;
    m.add_function(wrap_pyfunction!(stdout, m)?)?;
    m.add_function(wrap_pyfunction!(memory_recording, m)?)?;
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    m.add_function(wrap_pyfunction!(disconnect, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;

    // time
    m.add_function(wrap_pyfunction!(set_time_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_nanos, m)?)?;
    m.add_function(wrap_pyfunction!(disable_timeline, m)?)?;
    m.add_function(wrap_pyfunction!(reset_time, m)?)?;

    // log any
    m.add_function(wrap_pyfunction!(log_arrow_msg, m)?)?;
    m.add_function(wrap_pyfunction!(log_file_from_path, m)?)?;
    m.add_function(wrap_pyfunction!(log_file_from_contents, m)?)?;

    // misc
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(get_app_url, m)?)?;
    m.add_function(wrap_pyfunction!(start_web_viewer_server, m)?)?;
    m.add_function(wrap_pyfunction!(escape_entity_path_part, m)?)?;
    m.add_function(wrap_pyfunction!(new_entity_path, m)?)?;

    // blueprint
    m.add_function(wrap_pyfunction!(set_panels, m)?)?;
    m.add_function(wrap_pyfunction!(add_space_view, m)?)?;
    m.add_function(wrap_pyfunction!(set_auto_space_views, m)?)?;

    Ok(())
}

// --- Init ---

#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::struct_excessive_bools)]
#[allow(clippy::too_many_arguments)]
#[pyfunction]
#[pyo3(signature = (
    application_id,
    recording_id=None,
    make_default=true,
    make_thread_default=true,
    application_path=None,
    default_enabled=true,
))]
fn new_recording(
    py: Python<'_>,
    application_id: String,
    recording_id: Option<String>,
    make_default: bool,
    make_thread_default: bool,
    application_path: Option<PathBuf>,
    default_enabled: bool,
) -> PyResult<PyRecordingStream> {
    // The sentinel file we use to identify the official examples directory.
    const SENTINEL_FILENAME: &str = ".rerun_examples";
    let is_official_example = application_path.map_or(false, |mut path| {
        // more than 4 layers would be really pushing it
        for _ in 0..4 {
            path.pop(); // first iteration is always a file path in our examples
            if path.join(SENTINEL_FILENAME).exists() {
                return true;
            }
        }
        false
    });

    let recording_id = if let Some(recording_id) = recording_id {
        StoreId::from_string(StoreKind::Recording, recording_id)
    } else {
        default_store_id(py, StoreKind::Recording, &application_id)
    };

    let mut batcher_config = re_log_types::DataTableBatcherConfig::from_env().unwrap_or_default();
    let on_release = |chunk| {
        GARBAGE_QUEUE.0.send(chunk).ok();
    };
    batcher_config.hooks.on_release = Some(on_release.into());

    let recording = RecordingStreamBuilder::new(application_id)
        .batcher_config(batcher_config)
        .is_official_example(is_official_example)
        .store_id(recording_id.clone())
        .store_source(re_log_types::StoreSource::PythonSdk(python_version(py)))
        .default_enabled(default_enabled)
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
    all_recordings().insert(recording_id, recording.clone());

    Ok(PyRecordingStream(recording))
}

#[allow(clippy::fn_params_excessive_bools)]
#[pyfunction]
#[pyo3(signature = (
    application_id,
    blueprint_id=None,
    make_default=true,
    make_thread_default=true,
    default_enabled=true,
))]
fn new_blueprint(
    py: Python<'_>,
    application_id: String,
    blueprint_id: Option<String>,
    make_default: bool,
    make_thread_default: bool,
    default_enabled: bool,
) -> PyResult<PyRecordingStream> {
    let blueprint_id = if let Some(blueprint_id) = blueprint_id {
        StoreId::from_string(StoreKind::Blueprint, blueprint_id)
    } else {
        default_store_id(py, StoreKind::Blueprint, &application_id)
    };

    let mut batcher_config = re_log_types::DataTableBatcherConfig::from_env().unwrap_or_default();
    let on_release = |chunk| {
        GARBAGE_QUEUE.0.send(chunk).ok();
    };
    batcher_config.hooks.on_release = Some(on_release.into());

    let blueprint = RecordingStreamBuilder::new(application_id)
        .batcher_config(batcher_config)
        .store_id(blueprint_id.clone())
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
    all_recordings().insert(blueprint_id, blueprint.clone());

    Ok(PyRecordingStream(blueprint))
}

#[pyfunction]
fn shutdown(py: Python<'_>) {
    re_log::debug!("Shutting down the Rerun SDK");
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        for (_, recording) in all_recordings().drain() {
            recording.disconnect();
        }
        flush_garbage_queue();
    });
}

// --- Recordings ---

#[pyclass(frozen)]
#[derive(Clone)]
struct PyRecordingStream(RecordingStream);

impl std::ops::Deref for PyRecordingStream {
    type Target = RecordingStream;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[pyfunction]
fn get_application_id(recording: Option<&PyRecordingStream>) -> Option<String> {
    get_data_recording(recording)?
        .store_info()
        .map(|info| info.application_id.to_string())
}

#[pyfunction]
fn get_recording_id(recording: Option<&PyRecordingStream>) -> Option<String> {
    get_data_recording(recording)?
        .store_info()
        .map(|info| info.store_id.to_string())
}

/// Returns the currently active data recording in the global scope, if any; fallbacks to the
/// specified recording otherwise, if any.
#[pyfunction]
fn get_data_recording(recording: Option<&PyRecordingStream>) -> Option<PyRecordingStream> {
    RecordingStream::get_quiet(
        rerun::StoreKind::Recording,
        recording.map(|rec| rec.0.clone()),
    )
    .map(PyRecordingStream)
}

/// Returns the currently active data recording in the global scope, if any.
#[pyfunction]
fn get_global_data_recording() -> Option<PyRecordingStream> {
    RecordingStream::global(rerun::StoreKind::Recording).map(PyRecordingStream)
}

/// Cleans up internal state if this is the child of a forked process.
#[pyfunction]
fn cleanup_if_forked_child() {
    rerun::cleanup_if_forked_child();
}

/// Replaces the currently active recording in the global scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
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
            rerun::StoreKind::Recording,
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
    RecordingStream::thread_local(rerun::StoreKind::Recording).map(PyRecordingStream)
}

/// Replaces the currently active recording in the thread-local scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
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
            rerun::StoreKind::Recording,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream);
        flush_garbage_queue();
        rec
    })
}

/// Returns the currently active blueprint recording in the global scope, if any; fallbacks to the
/// specified recording otherwise, if any.
#[pyfunction]
fn get_blueprint_recording(overrides: Option<&PyRecordingStream>) -> Option<PyRecordingStream> {
    RecordingStream::get_quiet(
        rerun::StoreKind::Blueprint,
        overrides.map(|rec| rec.0.clone()),
    )
    .map(PyRecordingStream)
}

/// Returns the currently active blueprint recording in the global scope, if any.
#[pyfunction]
fn get_global_blueprint_recording() -> Option<PyRecordingStream> {
    RecordingStream::global(rerun::StoreKind::Blueprint).map(PyRecordingStream)
}

/// Replaces the currently active recording in the global scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
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
            rerun::StoreKind::Blueprint,
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
    RecordingStream::thread_local(rerun::StoreKind::Blueprint).map(PyRecordingStream)
}

/// Replaces the currently active recording in the thread-local scope with the specified one.
///
/// Returns the previous one, if any.
#[pyfunction]
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
            rerun::StoreKind::Blueprint,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream);
        flush_garbage_queue();
        rec
    })
}

// --- Sinks ---

#[pyfunction]
fn is_enabled(recording: Option<&PyRecordingStream>) -> bool {
    get_data_recording(recording).map_or(false, |rec| rec.is_enabled())
}

#[pyfunction]
#[pyo3(signature = (addr = None, flush_timeout_sec=rerun::default_flush_timeout().unwrap().as_secs_f32(), recording = None))]
fn connect(
    addr: Option<String>,
    flush_timeout_sec: Option<f32>,
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> PyResult<()> {
    let addr = if let Some(addr) = addr {
        addr.parse()?
    } else {
        rerun::default_server_addr()
    };

    let flush_timeout = flush_timeout_sec.map(std::time::Duration::from_secs_f32);

    // The call to connect may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        if let Some(recording) = recording {
            // If the user passed in a recording, use it
            recording.connect_opts(addr, flush_timeout);
        } else {
            // Otherwise, connect both global defaults
            if let Some(recording) = get_data_recording(None) {
                recording.connect_opts(addr, flush_timeout);
            };
            if let Some(blueprint) = get_blueprint_recording(None) {
                blueprint.connect_opts(addr, flush_timeout);
            };
        }
        flush_garbage_queue();
    });

    Ok(())
}

#[pyfunction]
#[pyo3(signature = (path, recording = None))]
fn save(path: &str, recording: Option<&PyRecordingStream>, py: Python<'_>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    // The call to save may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        let res = recording
            .save(path)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()));
        flush_garbage_queue();
        res
    })
}

#[pyfunction]
#[pyo3(signature = (recording = None))]
fn stdout(recording: Option<&PyRecordingStream>, py: Python<'_>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    // The call to stdout may internally flush.
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        let res = recording
            .stdout()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()));
        flush_garbage_queue();
        res
    })
}

/// Create an in-memory rrd file
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
        PyMemorySinkStorage { rec: rec.0, inner }
    })
}

#[pyclass(frozen)]
struct PyMemorySinkStorage {
    // So we can flush when needed!
    rec: RecordingStream,
    inner: MemorySinkStorage,
}

#[pymethods]
impl PyMemorySinkStorage {
    /// Concatenate the contents of the [`MemorySinkStorage`] as byes.
    ///
    /// Note: This will do a blocking flush before returning!
    fn concat_as_bytes<'p>(
        &self,
        concat: Option<&PyMemorySinkStorage>,
        py: Python<'p>,
    ) -> PyResult<&'p PyBytes> {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            self.rec.flush_blocking();
            flush_garbage_queue();
        });

        MemorySinkStorage::concat_memory_sinks_as_bytes(
            [Some(&self.inner), concat.map(|c| &c.inner)]
                .iter()
                .filter_map(|s| *s)
                .collect_vec()
                .as_slice(),
        )
        .map(|bytes| PyBytes::new(py, bytes.as_slice()))
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }

    /// Count the number of pending messages in the [`MemorySinkStorage`].
    ///
    /// This will do a blocking flush before returning!
    fn num_msgs(&self, py: Python<'_>) -> usize {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            self.rec.flush_blocking();
            flush_garbage_queue();
        });

        self.inner.num_msgs()
    }
}

#[cfg(feature = "web_viewer")]
#[must_use = "the tokio_runtime guard must be kept alive while using tokio"]
fn enter_tokio_runtime() -> tokio::runtime::EnterGuard<'static> {
    static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
        Lazy::new(|| tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"));
    TOKIO_RUNTIME.enter()
}

/// Serve a web-viewer.
#[allow(clippy::unnecessary_wraps)] // False positive
#[pyfunction]
#[pyo3(signature = (open_browser, web_port, ws_port, server_memory_limit, recording = None))]
fn serve(
    open_browser: bool,
    web_port: Option<u16>,
    ws_port: Option<u16>,
    server_memory_limit: String,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    #[cfg(feature = "web_viewer")]
    {
        let Some(recording) = get_data_recording(recording) else {
            return Ok(());
        };

        let _guard = enter_tokio_runtime();

        let server_memory_limit = re_memory::MemoryLimit::parse(&server_memory_limit)
            .map_err(|err| PyRuntimeError::new_err(format!("Bad server_memory_limit: {err}:")))?;

        recording.set_sink(
            rerun::web_viewer::new_sink(
                open_browser,
                "0.0.0.0",
                web_port.map(WebViewerServerPort).unwrap_or_default(),
                ws_port.map(RerunServerPort).unwrap_or_default(),
                server_memory_limit,
            )
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
        );

        Ok(())
    }

    #[cfg(not(feature = "web_viewer"))]
    {
        _ = recording;
        _ = web_port;
        _ = ws_port;
        _ = open_browser;
        _ = server_memory_limit;
        Err(PyRuntimeError::new_err(
            "The Rerun SDK was not compiled with the 'web_viewer' feature",
        ))
    }
}

/// Disconnect from remote server (if any).
///
/// Subsequent log messages will be buffered and either sent on the next call to `connect`,
/// or shown with `show`.
#[pyfunction]
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

/// Block until outstanding data has been flushed to the sink
#[pyfunction]
fn flush(py: Python<'_>, blocking: bool, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        if blocking {
            recording.flush_blocking();
        } else {
            recording.flush_async();
        }
        flush_garbage_queue();
    });
}

// --- Time ---

#[pyfunction]
fn set_time_sequence(timeline: &str, sequence: i64, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time_sequence(timeline, sequence);
}

#[pyfunction]
fn set_time_seconds(timeline: &str, seconds: f64, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time_seconds(timeline, seconds);
}

#[pyfunction]
fn set_time_nanos(timeline: &str, nanos: i64, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time_nanos(timeline, nanos);
}

#[pyfunction]
fn disable_timeline(timeline: &str, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.disable_timeline(timeline);
}

#[pyfunction]
fn reset_time(recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.reset_time();
}

// --- Log special ---

#[pyfunction]
fn set_panels(
    blueprint_view_expanded: Option<bool>,
    selection_view_expanded: Option<bool>,
    timeline_view_expanded: Option<bool>,
    blueprint: Option<&PyRecordingStream>,
) {
    // TODO(jleibs): This should go away as part of https://github.com/rerun-io/rerun/issues/2089
    use re_viewer::blueprint::components::PanelView;

    if let Some(expanded) = blueprint_view_expanded {
        set_panel(PanelView::BLUEPRINT_VIEW_PATH, expanded, blueprint);
    }
    if let Some(expanded) = selection_view_expanded {
        set_panel(PanelView::SELECTION_VIEW_PATH, expanded, blueprint);
    }
    if let Some(expanded) = timeline_view_expanded {
        set_panel(PanelView::TIMELINE_VIEW_PATH, expanded, blueprint);
    }
}

fn set_panel(entity_path: &str, is_expanded: bool, blueprint: Option<&PyRecordingStream>) {
    let Some(blueprint) = get_blueprint_recording(blueprint) else {
        return;
    };

    // TODO(jleibs): This should go away as part of https://github.com/rerun-io/rerun/issues/2089
    use re_viewer::blueprint::components::PanelView;

    // TODO(jleibs): Validation this is a valid blueprint path?
    let entity_path = EntityPath::parse_forgiving(entity_path);

    let panel_state = PanelView(is_expanded);

    let row = DataRow::from_cells1(
        RowId::new(),
        entity_path,
        TimePoint::default(),
        1,
        [panel_state].as_slice(),
    )
    .unwrap(); // Can only fail if we have the wrong number of instances for the component, and we don't

    // TODO(jleibs) timeless? Something else?
    let timeless = true;
    blueprint.record_row(row, !timeless);
}

#[pyfunction]
fn add_space_view(
    _name: &str,
    _space_view_class: &str,
    _origin: &str,
    _entity_paths: Vec<&str>,
    _blueprint: Option<&PyRecordingStream>,
) -> PyResult<()> {
    Err(PyRuntimeError::new_err(
        "add_space_view is broken until blueprint refactoring is complete: https://github.com/rerun-io/rerun/issues/4167",
    ))

    /*
    let Some(blueprint) = get_blueprint_recording(blueprint) else {
        return;
    };

    let entity_paths = entity_paths.into_iter().map(|s| s.into()).collect_vec();
    let mut space_view =
        SpaceViewBlueprint::new(space_view_class.into(), &origin.into(), entity_paths.iter());

    // Choose the space-view id deterministically from the name; this means the user
    // can run the application multiple times and get sane behavior.
    space_view.id = SpaceViewId::hashed_from_str(name);

    space_view.display_name = name.into();
    space_view.entities_determined_by_user = true;

    let entity_path = space_view.entity_path();

    let space_view = SpaceViewComponent { space_view };

    let row = DataRow::from_cells1(
        RowId::new(),
        entity_path,
        TimePoint::default(),
        1,
        [space_view].as_slice(),
    )
    .unwrap();

    // TODO(jleibs) timeless? Something else?
    let timeless = true;
    blueprint.record_row(row, !timeless);
    */
}

#[pyfunction]
fn set_auto_space_views(enabled: bool, blueprint: Option<&PyRecordingStream>) {
    let Some(blueprint) = get_blueprint_recording(blueprint) else {
        return;
    };

    // TODO(jleibs): This should go away as part of https://github.com/rerun-io/rerun/issues/2089
    use re_viewport::blueprint::components::AutoSpaceViews;

    let enable_auto_space = AutoSpaceViews(enabled);

    let row = DataRow::from_cells1(
        RowId::new(),
        VIEWPORT_PATH,
        TimePoint::default(),
        1,
        [enable_auto_space].as_slice(),
    )
    .unwrap();

    // TODO(jleibs) timeless? Something else?
    let timeless = true;
    blueprint.record_row(row, !timeless);
}

#[pyfunction]
#[pyo3(signature = (
    entity_path,
    components,
    timeless,
    recording=None,
))]
fn log_arrow_msg(
    py: Python<'_>,
    entity_path: &str,
    components: &PyDict,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let entity_path = EntityPath::parse_forgiving(entity_path);

    // It's important that we don't hold the session lock while building our arrow component.
    // the API we call to back through pyarrow temporarily releases the GIL, which can cause
    // a deadlock.
    let row = crate::arrow::build_data_row_from_components(
        &entity_path,
        components,
        &TimePoint::default(),
    )?;

    recording.record_row(row, !timeless);

    py.allow_threads(flush_garbage_queue);

    Ok(())
}

#[pyfunction]
#[pyo3(signature = (
    file_path,
    recording=None,
))]
fn log_file_from_path(
    py: Python<'_>,
    file_path: std::path::PathBuf,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let Some(recording_id) = recording.store_info().map(|info| info.store_id.clone()) else {
        return Ok(());
    };
    let settings = rerun::DataLoaderSettings::recommended(recording_id);

    recording
        .log_file_from_path(&settings, file_path)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    py.allow_threads(flush_garbage_queue);

    Ok(())
}

#[pyfunction]
#[pyo3(signature = (
    file_path,
    file_contents,
    recording=None,
))]
fn log_file_from_contents(
    py: Python<'_>,
    file_path: std::path::PathBuf,
    file_contents: &[u8],
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let Some(recording_id) = recording.store_info().map(|info| info.store_id.clone()) else {
        return Ok(());
    };
    let settings = rerun::DataLoaderSettings::recommended(recording_id);

    recording
        .log_file_from_contents(
            &settings,
            file_path,
            std::borrow::Cow::Borrowed(file_contents),
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    py.allow_threads(flush_garbage_queue);

    Ok(())
}

// --- Misc ---

/// Return a verbose version string
#[pyfunction]
fn version() -> String {
    re_build_info::build_info!().to_string()
}

/// Get a url to an instance of the web-viewer
///
/// This may point to app.rerun.io or localhost depending on
/// whether `host_assets` was called.
#[pyfunction]
fn get_app_url() -> String {
    #[cfg(feature = "web_viewer")]
    if let Some(hosted_assets) = &*global_web_viewer_server() {
        return hosted_assets.server_url();
    }

    let build_info = re_build_info::build_info!();
    let short_git_hash = &build_info.git_hash[..7];
    format!("https://app.rerun.io/commit/{short_git_hash}")
}

// TODO(jleibs) expose this as a python type
/// Start a web server to host the run web-assets
#[pyfunction]
fn start_web_viewer_server(port: u16) -> PyResult<()> {
    #[allow(clippy::unnecessary_wraps)]
    #[cfg(feature = "web_viewer")]
    {
        let mut web_handle = global_web_viewer_server();

        let _guard = enter_tokio_runtime();
        *web_handle = Some(
            re_web_viewer_server::WebViewerServerHandle::new("0.0.0.0", WebViewerServerPort(port))
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

#[pyfunction]
fn escape_entity_path_part(part: &str) -> String {
    EntityPathPart::from(part).escaped_string()
}

#[pyfunction]
fn new_entity_path(parts: Vec<&str>) -> String {
    let path = EntityPath::from(parts.into_iter().map(EntityPathPart::from).collect_vec());
    path.to_string()
}

// --- Helpers ---

fn python_version(py: Python<'_>) -> re_log_types::PythonVersion {
    let py_version = py.version_info();
    re_log_types::PythonVersion {
        major: py_version.major,
        minor: py_version.minor,
        patch: py_version.patch,
        suffix: py_version.suffix.map(|s| s.to_owned()).unwrap_or_default(),
    }
}

fn default_store_id(py: Python<'_>, variant: StoreKind, application_id: &str) -> StoreId {
    use rand::{Rng as _, SeedableRng as _};
    use std::hash::{Hash as _, Hasher as _};

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
            re_log::error_once!("Failed to retrieve python authkey: {err}\nMultiprocessing will result in split recordings.");
            // If authkey failed, just generate a random 8-byte authkey
            let bytes = rand::Rng::gen::<[u8; 8]>(&mut rand::thread_rng());
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
    //
    // Keep in mind: application IDs are merely metadata, everything is the store/viewer is driven
    // solely by recording IDs.
    application_id.hash(&mut hasher);
    let mut rng = rand::rngs::StdRng::seed_from_u64(hasher.finish());
    let uuid = uuid::Builder::from_random_bytes(rng.gen()).into_uuid();
    StoreId::from_uuid(variant, uuid)
}

fn authkey(py: Python<'_>) -> PyResult<Vec<u8>> {
    let locals = PyDict::new(py);

    py.run(
        r#"
import multiprocessing
# authkey is the same for child and parent processes, so this is how we know we're the same
authkey = multiprocessing.current_process().authkey
            "#,
        None,
        Some(locals),
    )
    .and_then(|()| {
        locals
            .get_item("authkey")?
            .ok_or_else(|| PyRuntimeError::new_err("authkey missing from expected locals"))
    })
    .and_then(|authkey| {
        authkey
            .downcast()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    })
    .map(|authkey: &PyBytes| authkey.as_bytes().to_vec())
}
