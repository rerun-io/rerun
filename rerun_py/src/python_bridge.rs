#![allow(clippy::borrow_deref_ref)] // False positive due to #[pyfunction] macro
#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![allow(clippy::too_many_arguments)] // We used named arguments, so this is fine
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use std::io::IsTerminal as _;
use std::{borrow::Borrow as _, collections::HashMap};

use arrow::array::RecordBatch as ArrowRecordBatch;
use itertools::Itertools as _;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyDict},
};

use re_log::ResultExt as _;
use re_log_types::LogMsg;
use re_log_types::{BlueprintActivationCommand, EntityPathPart, StoreKind};
use re_sdk::send_file_to_sink;
use re_sdk::sink::CallbackSink;
use re_sdk::{external::re_log_encoding::encoder::encode_ref_as_bytes_local, TimeCell};
use re_sdk::{
    sink::{BinaryStreamStorage, MemorySinkStorage},
    time::TimePoint,
    EntityPath, RecordingStream, RecordingStreamBuilder, StoreId,
};

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

use once_cell::sync::{Lazy, OnceCell};

use crate::dataframe::PyRecording;

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
) -> parking_lot::MutexGuard<'static, Option<re_web_viewer_server::WebViewerServer>> {
    static WEB_HANDLE: OnceCell<parking_lot::Mutex<Option<re_web_viewer_server::WebViewerServer>>> =
        OnceCell::new();
    WEB_HANDLE.get_or_init(Default::default).lock()
}

/// The python module is called "rerun_bindings".
#[pymodule]
fn rerun_bindings(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // NOTE: We do this here because some the inner init methods don't respond too kindly to being
    // called more than once.
    // The SDK should not be as noisy as the CLI, so we set log filter to warning if not specified otherwise.
    re_log::setup_logging_with_filter(&re_log::log_filter_from_env_or_default("warn"));

    // These two components are necessary for imports to work
    m.add_class::<PyMemorySinkStorage>()?;
    m.add_class::<PyRecordingStream>()?;
    m.add_class::<PyBinarySinkStorage>()?;

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
    m.add_function(wrap_pyfunction!(spawn, m)?)?;

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
    m.add_function(wrap_pyfunction!(binary_stream, m)?)?;
    m.add_function(wrap_pyfunction!(connect_grpc, m)?)?;
    m.add_function(wrap_pyfunction!(connect_grpc_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(save, m)?)?;
    m.add_function(wrap_pyfunction!(save_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(stdout, m)?)?;
    m.add_function(wrap_pyfunction!(memory_recording, m)?)?;
    m.add_function(wrap_pyfunction!(set_callback_sink, m)?)?;
    m.add_function(wrap_pyfunction!(serve_grpc, m)?)?;
    m.add_function(wrap_pyfunction!(serve_web, m)?)?;
    m.add_function(wrap_pyfunction!(disconnect, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;

    // time
    m.add_function(wrap_pyfunction!(set_time_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_duration_nanos, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_timestamp_nanos_since_epoch, m)?)?;
    m.add_function(wrap_pyfunction!(disable_timeline, m)?)?;
    m.add_function(wrap_pyfunction!(reset_time, m)?)?;

    // log any
    m.add_function(wrap_pyfunction!(log_arrow_msg, m)?)?;
    m.add_function(wrap_pyfunction!(log_file_from_path, m)?)?;
    m.add_function(wrap_pyfunction!(log_file_from_contents, m)?)?;
    m.add_function(wrap_pyfunction!(send_arrow_chunk, m)?)?;
    m.add_function(wrap_pyfunction!(send_blueprint, m)?)?;
    m.add_function(wrap_pyfunction!(send_recording, m)?)?;

    // misc
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(get_app_url, m)?)?;
    m.add_function(wrap_pyfunction!(start_web_viewer_server, m)?)?;
    m.add_function(wrap_pyfunction!(escape_entity_path_part, m)?)?;
    m.add_function(wrap_pyfunction!(new_entity_path, m)?)?;
    m.add_function(wrap_pyfunction!(dataloader_bytes_from_path_to_callback, m)?)?;

    // properties
    m.add_function(wrap_pyfunction!(new_property_entity_path, m)?)?;
    m.add_function(wrap_pyfunction!(send_recording_name, m)?)?;
    m.add_function(wrap_pyfunction!(send_recording_start_time_nanos, m)?)?;

    use crate::video::asset_video_read_frame_timestamps_nanos;
    m.add_function(wrap_pyfunction!(
        asset_video_read_frame_timestamps_nanos,
        m
    )?)?;

    // dataframes
    crate::dataframe::register(m)?;

    // catalog
    crate::catalog::register(py, m)?;

    Ok(())
}

// --- Init ---

/// Create a new recording stream.
#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::struct_excessive_bools)]
#[pyfunction]
#[pyo3(signature = (
    application_id,
    recording_id=None,
    make_default=true,
    make_thread_default=true,
    default_enabled=true,
    send_properties=true,
))]
fn new_recording(
    py: Python<'_>,
    application_id: String,
    recording_id: Option<String>,
    make_default: bool,
    make_thread_default: bool,
    default_enabled: bool,
    send_properties: bool,
) -> PyResult<PyRecordingStream> {
    let recording_id = if let Some(recording_id) = recording_id {
        StoreId::from_string(StoreKind::Recording, recording_id)
    } else {
        default_store_id(py, StoreKind::Recording, &application_id)
    };

    let mut batcher_config = re_chunk::ChunkBatcherConfig::from_env().unwrap_or_default();
    let on_release = |chunk| {
        GARBAGE_QUEUE.0.send(chunk).ok();
    };
    batcher_config.hooks.on_release = Some(on_release.into());

    let recording = RecordingStreamBuilder::new(application_id)
        .batcher_config(batcher_config)
        .store_id(recording_id.clone())
        .store_source(re_log_types::StoreSource::PythonSdk(python_version(py)))
        .default_enabled(default_enabled)
        .send_properties(send_properties)
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

/// Create a new blueprint stream.
#[allow(clippy::fn_params_excessive_bools)]
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
    // We don't currently support additive blueprints, so we should always be generating a new, unique
    // blueprint id to avoid collisions.
    let blueprint_id = StoreId::random(StoreKind::Blueprint);

    let mut batcher_config = re_chunk::ChunkBatcherConfig::from_env().unwrap_or_default();
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
        for (_, recording) in all_recordings().iter() {
            recording.disconnect();
        }

        flush_garbage_queue();
    });
}

// --- Recordings ---

#[pyclass(frozen)]
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
        .map(|info| info.application_id.to_string())
}

/// Get the current recording stream's recording ID.
#[pyfunction]
#[pyo3(signature = (recording=None))]
fn get_recording_id(recording: Option<&PyRecordingStream>) -> Option<String> {
    get_data_recording(recording)?
        .store_info()
        .map(|info| info.store_id.to_string())
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
#[pyo3(signature = (port = 9876, memory_limit = "75%".to_owned(), hide_welcome_screen = false, detach_process = true, executable_name = "rerun".to_owned(), executable_path = None, extra_args = vec![], extra_env = vec![]))]
fn spawn(
    port: u16,
    memory_limit: String,
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
        hide_welcome_screen,
        detach_process,
        executable_name,
        executable_path,
        extra_args,
        extra_env,
    };

    re_sdk::spawn(&spawn_opts).map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

/// Connect the recording stream to a remote Rerun Viewer on the given HTTP(S) URL.
#[pyfunction]
#[pyo3(signature = (url, flush_timeout_sec=re_sdk::default_flush_timeout().expect("always Some()").as_secs_f32(), default_blueprint = None, recording = None))]
fn connect_grpc(
    url: Option<String>,
    flush_timeout_sec: Option<f32>,
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
    py: Python<'_>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let url = url.unwrap_or_else(|| re_sdk::DEFAULT_CONNECT_URL.to_owned());
    let endpoint = url
        .parse::<re_uri::ProxyEndpoint>()
        .map_err(|err| PyRuntimeError::wrap(err, format!("invalid endpoint {url:?}")))?;

    if re_sdk::forced_sink_path().is_some() {
        re_log::debug!("Ignored call to `connect()` since _RERUN_TEST_FORCE_SAVE is set");
        return Ok(());
    }

    py.allow_threads(|| {
        let sink = re_sdk::sink::GrpcSink::new(
            endpoint,
            flush_timeout_sec.map(std::time::Duration::from_secs_f32),
        );

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

    if let Some(blueprint_id) = (*blueprint_stream).store_info().map(|info| info.store_id) {
        // The call to save, needs to flush.
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            // Flush all the pending blueprint messages before we include the Ready message
            blueprint_stream.flush_blocking();

            let activation_cmd = BlueprintActivationCommand {
                blueprint_id,
                make_active,
                make_default,
            };

            blueprint_stream.record_msg(activation_cmd.into());

            blueprint_stream
                .connect_grpc_opts(url, None)
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
        py.allow_threads(|| {
            // Flush all the pending blueprint messages before we include the Ready message
            blueprint_stream.flush_blocking();

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
            let data = encode_ref_as_bytes_local(msgs.iter().map(Ok)).ok_or_log_error()?;
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

#[pyclass(frozen)]
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
}

#[pyclass(frozen)]
struct PyBinarySinkStorage {
    /// The underlying binary sink storage.
    inner: BinaryStreamStorage,
}

#[pymethods]
impl PyBinarySinkStorage {
    /// Read the bytes from the binary sink.
    ///
    /// If `flush` is `true`, the sink will be flushed before reading.
    #[pyo3(signature = (*, flush = true))]
    fn read<'p>(&self, flush: bool, py: Python<'p>) -> Bound<'p, PyBytes> {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        PyBytes::new(
            py,
            py.allow_threads(|| {
                if flush {
                    self.inner.flush();
                }

                let bytes = self.inner.read();

                flush_garbage_queue();

                bytes
            })
            .as_slice(),
        )
    }

    /// Flush the binary sink manually.
    fn flush(&self, py: Python<'_>) {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            self.inner.flush();
            flush_garbage_queue();
        });
    }
}

/// Spawn a gRPC server which an SDK or Viewer can connect to.
#[pyfunction]
#[pyo3(signature = (grpc_port, server_memory_limit, default_blueprint = None, recording = None))]
fn serve_grpc(
    grpc_port: Option<u16>,
    server_memory_limit: String,
    default_blueprint: Option<&PyMemorySinkStorage>,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    #[cfg(feature = "server")]
    {
        let Some(recording) = get_data_recording(recording) else {
            return Ok(());
        };

        if re_sdk::forced_sink_path().is_some() {
            re_log::debug!("Ignored call to `serve_grpc()` since _RERUN_TEST_FORCE_SAVE is set");
            return Ok(());
        }

        let server_memory_limit = re_memory::MemoryLimit::parse(&server_memory_limit)
            .map_err(|err| PyRuntimeError::new_err(format!("Bad server_memory_limit: {err}:")))?;

        let sink = re_sdk::grpc_server::GrpcServerSink::new(
            "0.0.0.0",
            grpc_port.unwrap_or(re_grpc_server::DEFAULT_SERVER_PORT),
            server_memory_limit,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        if let Some(default_blueprint) = default_blueprint {
            send_mem_sink_as_default_blueprint(&sink, default_blueprint);
        }

        recording.set_sink(Box::new(sink));

        Ok(())
    }

    #[cfg(not(feature = "server"))]
    {
        let _ = (grpc_port, server_memory_limit, default_blueprint, recording);

        Err(PyRuntimeError::new_err(
            "The Rerun SDK was not compiled with the 'server' feature",
        ))
    }
}

/// Serve a web-viewer.
#[allow(clippy::unnecessary_wraps)] // False positive
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

        let server_memory_limit = re_memory::MemoryLimit::parse(&server_memory_limit)
            .map_err(|err| PyRuntimeError::new_err(format!("Bad server_memory_limit: {err}:")))?;

        let sink = re_sdk::web_viewer::new_sink(
            open_browser,
            "0.0.0.0",
            web_port.map(WebViewerServerPort).unwrap_or_default(),
            grpc_port.unwrap_or(re_grpc_server::DEFAULT_SERVER_PORT),
            server_memory_limit,
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
/// Subsequent log messages will be buffered and either sent on the next call to `connect`,
/// or shown with `show`.
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
#[pyo3(signature = (blocking, recording=None))]
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

    // It's important that we don't hold the session lock while building our arrow component.
    // the API we call to back through pyarrow temporarily releases the GIL, which can cause
    // a deadlock.
    let row = crate::arrow::build_row_from_components(&components, &TimePoint::default())?;

    recording.record_row(entity_path, row, !static_);

    py.allow_threads(flush_garbage_queue);

    Ok(())
}

/// Directly send an arrow chunk to the recording stream.
///
/// Params
/// ------
/// entity_path: `str`
///     The entity path to log the chunk to.
/// timelines: `Dict[str, arrow::Int64Array]`
///     A dictionary mapping timeline names to their values.
/// components: `Dict[str, arrow::ListArray]`
///     A dictionary mapping component names to their values.
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

    recording.send_chunk(chunk);

    py.allow_threads(flush_garbage_queue);

    Ok(())
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
    for chunk in store.iter_chunks() {
        recording.send_chunk((**chunk).clone());
    }
}

// --- Misc ---

/// Return a verbose version string.
#[pyfunction]
fn version() -> String {
    re_build_info::build_info!().to_string()
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
#[pyfunction]
fn start_web_viewer_server(port: u16) -> PyResult<()> {
    #[allow(clippy::unnecessary_wraps)]
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

#[pyfunction]
fn dataloader_bytes_from_path_to_callback(
    file_path: std::path::PathBuf,
    callback: PyObject,
    py: Python<'_>,
) -> PyResult<()> {
    let callback = move |msgs: &[LogMsg]| {
        Python::with_gil(|py| {
            let data = encode_ref_as_bytes_local(msgs.iter().map(Ok)).ok_or_log_error()?;
            let bytes = PyBytes::new(py, &data);
            callback.bind(py).call1((bytes,)).ok_or_log_error()?;
            Some(())
        });
    };

    let callback_sink = CallbackSink::new(callback);

    py.allow_threads(|| {
        send_file_to_sink(file_path, &callback_sink)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    })
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
