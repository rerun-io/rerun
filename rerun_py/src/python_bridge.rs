#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use std::{collections::HashMap, path::PathBuf};

use itertools::Itertools;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyDict},
};

use re_viewer_context::SpaceViewId;
use re_viewport::{
    blueprint_components::{AutoSpaceViews, SpaceViewComponent, VIEWPORT_PATH},
    SpaceViewBlueprint,
};

use re_log_types::{DataRow, StoreKind};
use rerun::{
    log::RowId, sink::MemorySinkStorage, time::TimePoint, EntityPath, RecordingStream,
    RecordingStreamBuilder, StoreId,
};

pub use rerun::{
    components::{
        AnnotationContext, Blob, ClassId, Color, DisconnectedSpace, DrawOrder, InstanceKey,
        KeypointId, LineStrip2D, LineStrip3D, OutOfTreeTransform3D, PinholeProjection, Position2D,
        Position3D, Radius, Text, Transform3D, Vector3D, ViewCoordinates,
    },
    coordinates::{Axis3, Handedness, Sign, SignedAxis3},
};

#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "web_viewer")]
use re_ws_comms::RerunServerPort;

// --- FFI ---

use once_cell::sync::OnceCell;
use parking_lot::Mutex;

// The bridge needs to have complete control over the lifetimes of the individual recordings,
// otherwise all the recording shutdown machinery (which includes deallocating C, Rust and Python
// data and joining a bunch of threads) can end up running at any time depending on what the
// Python GC is doing, which obviously leads to very bad things :tm:.
//
// TODO(#2116): drop unused recordings
fn all_recordings() -> parking_lot::MutexGuard<'static, HashMap<StoreId, RecordingStream>> {
    static ALL_RECORDINGS: OnceCell<Mutex<HashMap<StoreId, RecordingStream>>> = OnceCell::new();
    ALL_RECORDINGS.get_or_init(Default::default).lock()
}

#[cfg(feature = "web_viewer")]
fn global_web_viewer_server(
) -> parking_lot::MutexGuard<'static, Option<re_web_viewer_server::WebViewerServerHandle>> {
    static WEB_HANDLE: OnceCell<Mutex<Option<re_web_viewer_server::WebViewerServerHandle>>> =
        OnceCell::new();
    WEB_HANDLE.get_or_init(Default::default).lock()
}

#[pyfunction]
fn main(py: Python<'_>, argv: Vec<String>) -> PyResult<u8> {
    let build_info = re_build_info::build_info!();
    let call_src = rerun::CallSource::Python(python_version(py));
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // Python catches SIGINT and waits for us to release the GIL before shtting down.
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
    re_log::setup_native_logging();

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
    m.add_function(wrap_pyfunction!(memory_recording, m)?)?;
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    m.add_function(wrap_pyfunction!(disconnect, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;

    // time
    m.add_function(wrap_pyfunction!(set_time_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_nanos, m)?)?;
    m.add_function(wrap_pyfunction!(reset_time, m)?)?;

    // log any
    m.add_function(wrap_pyfunction!(log_arrow_msg, m)?)?;

    // misc
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(get_app_url, m)?)?;
    m.add_function(wrap_pyfunction!(start_web_viewer_server, m)?)?;

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

    let recording = RecordingStreamBuilder::new(application_id)
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

    let blueprint = RecordingStreamBuilder::new(application_id)
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
    });
}

// --- Recordings ---

#[pyclass(frozen)]
#[derive(Clone)]
struct PyRecordingStream(RecordingStream);

impl std::ops::Deref for PyRecordingStream {
    type Target = RecordingStream;

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
        RecordingStream::set_global(
            rerun::StoreKind::Recording,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream)
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
        RecordingStream::set_thread_local(
            rerun::StoreKind::Recording,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream)
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
        RecordingStream::set_global(
            rerun::StoreKind::Blueprint,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream)
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
        RecordingStream::set_thread_local(
            rerun::StoreKind::Blueprint,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream)
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
) -> PyResult<()> {
    let addr = if let Some(addr) = addr {
        addr.parse()?
    } else {
        rerun::default_server_addr()
    };

    let flush_timeout = flush_timeout_sec.map(std::time::Duration::from_secs_f32);

    if let Some(recording) = recording {
        // If the user passed in a recording, use it
        recording.connect(addr, flush_timeout);
    } else {
        // Otherwise, connect both global defaults
        if let Some(recording) = get_data_recording(None) {
            recording.connect(addr, flush_timeout);
        };
        if let Some(blueprint) = get_blueprint_recording(None) {
            blueprint.connect(addr, flush_timeout);
        };
    }

    Ok(())
}

#[pyfunction]
#[pyo3(signature = (path, recording = None))]
fn save(path: &str, recording: Option<&PyRecordingStream>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    recording
        .save(path)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

/// Create an in-memory rrd file
#[pyfunction]
#[pyo3(signature = (recording = None))]
fn memory_recording(recording: Option<&PyRecordingStream>) -> Option<PyMemorySinkStorage> {
    get_data_recording(recording).map(|rec| {
        let inner = rec.memory();
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
        });

        self.inner.num_msgs()
    }
}

#[cfg(feature = "web_viewer")]
#[must_use = "the tokio_runtime guard must be kept alive while using tokio"]
fn enter_tokio_runtime() -> tokio::runtime::EnterGuard<'static> {
    use once_cell::sync::Lazy;
    static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
        Lazy::new(|| tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"));
    TOKIO_RUNTIME.enter()
}

/// Serve a web-viewer.
#[allow(clippy::unnecessary_wraps)] // False positive
#[pyfunction]
#[pyo3(signature = (open_browser, web_port, ws_port, recording = None))]
fn serve(
    open_browser: bool,
    web_port: Option<u16>,
    ws_port: Option<u16>,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    #[cfg(feature = "web_viewer")]
    {
        let Some(recording) = get_data_recording(recording) else {
            return Ok(());
        };

        let _guard = enter_tokio_runtime();

        recording.set_sink(
            rerun::web_viewer::new_sink(
                open_browser,
                "0.0.0.0",
                web_port.map(WebViewerServerPort).unwrap_or_default(),
                ws_port.map(RerunServerPort).unwrap_or_default(),
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
    });
}

// --- Time ---

#[pyfunction]
fn set_time_sequence(timeline: &str, sequence: Option<i64>, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time_sequence(timeline, sequence);
}

#[pyfunction]
fn set_time_seconds(timeline: &str, seconds: Option<f64>, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time_seconds(timeline, seconds);
}

#[pyfunction]
fn set_time_nanos(timeline: &str, nanos: Option<i64>, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else {
        return;
    };
    recording.set_time_nanos(timeline, nanos);
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
    use rerun::external::re_types::blueprint::PanelView;

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

    use rerun::external::re_types::blueprint::PanelView;

    // TODO(jleibs): Validation this is a valid blueprint path?
    let entity_path = parse_entity_path(entity_path);

    let panel_state = PanelView { is_expanded };

    let row = DataRow::from_cells1(
        RowId::random(),
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
    name: &str,
    space_view_class: &str,
    origin: &str,
    entity_paths: Vec<&str>,
    blueprint: Option<&PyRecordingStream>,
) {
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

    let entity_path = parse_entity_path(
        format!("{}/{}", SpaceViewComponent::SPACEVIEW_PREFIX, space_view.id).as_str(),
    );

    let space_view = SpaceViewComponent { space_view };

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        TimePoint::default(),
        1,
        [space_view].as_slice(),
    )
    .unwrap();

    // TODO(jleibs) timeless? Something else?
    let timeless = true;
    blueprint.record_row(row, !timeless);
}

#[pyfunction]
fn set_auto_space_views(enabled: bool, blueprint: Option<&PyRecordingStream>) {
    let Some(blueprint) = get_blueprint_recording(blueprint) else {
        return;
    };

    let enable_auto_space = AutoSpaceViews(enabled);

    let row = DataRow::from_cells1(
        RowId::random(),
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
    entity_path: &str,
    components: &PyDict,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let entity_path = parse_entity_path(entity_path);

    // It's important that we don't hold the session lock while building our arrow component.
    // the API we call to back through pyarrow temporarily releases the GIL, which can cause
    // cause a deadlock.
    let row = crate::arrow::build_data_row_from_components(
        &entity_path,
        components,
        &TimePoint::default(),
    )?;

    recording.record_row(row, !timeless);

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
            .get_item("authkey")
            .ok_or_else(|| PyRuntimeError::new_err("authkey missing from expected locals"))
    })
    .and_then(|authkey| {
        authkey
            .downcast()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    })
    .map(|authkey: &PyBytes| authkey.as_bytes().to_vec())
}

fn parse_entity_path(entity_path: &str) -> EntityPath {
    // We accept anything!
    EntityPath::parse_forgiving(entity_path)
}
