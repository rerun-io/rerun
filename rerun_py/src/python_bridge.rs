#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use itertools::izip;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyBytes, PyDict},
};

use re_log_types::{DataRow, RecordingType};
use rerun::{
    log::{PathOp, RowId},
    sink::MemorySinkStorage,
    time::TimePoint,
    EntityPath, RecordingId, RecordingStream, RecordingStreamBuilder,
};

pub use rerun::{
    components::{
        AnnotationContext, AnnotationInfo, Arrow3D, Box3D, ClassDescription, ClassId, ColorRGBA,
        DrawOrder, EncodedMesh3D, InstanceKey, KeypointId, Label, LineStrip2D, LineStrip3D, Mat3x3,
        Mesh3D, MeshFormat, MeshId, Pinhole, Point2D, Point3D, Quaternion, Radius, RawMesh3D,
        Rect2D, Rigid3, Scalar, ScalarPlotProps, Size3D, Tensor, TensorData, TensorDimension,
        TensorId, TextEntry, Transform, Vec2D, Vec3D, Vec4D, ViewCoordinates,
    },
    coordinates::{Axis3, Handedness, Sign, SignedAxis3},
};

#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "web_viewer")]
use re_ws_comms::RerunServerPort;

use crate::arrow::get_registered_component_names;

// --- FFI ---

use once_cell::sync::OnceCell;
use parking_lot::Mutex;

// The bridge needs to have complete control over the lifetimes of the individual recordings,
// otherwise all the recording shutdown machinery (which includes deallocating C, Rust and Python
// data and joining a bunch of threads) can end up running at any time depending on what the
// Python GC is doing, which obviously leads to very bad things :tm:.
//
// TODO(#2116): drop unused recordings
fn all_recordings() -> parking_lot::MutexGuard<'static, HashMap<RecordingId, RecordingStream>> {
    static ALL_RECORDINGS: OnceCell<Mutex<HashMap<RecordingId, RecordingStream>>> = OnceCell::new();
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
        .block_on(rerun::run(build_info, call_src, argv))
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
    // TODO(jleibs): Refactor import logic so all we need is main
    m.add_function(wrap_pyfunction!(get_registered_component_names, m)?)?;
    m.add_class::<TensorDataMeaning>()?;
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
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;

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

    // legacy log functions not yet ported to pure python
    m.add_function(wrap_pyfunction!(log_annotation_context, m)?)?;
    m.add_function(wrap_pyfunction!(log_arrow_msg, m)?)?;
    m.add_function(wrap_pyfunction!(log_cleared, m)?)?;
    m.add_function(wrap_pyfunction!(log_image_file, m)?)?;
    m.add_function(wrap_pyfunction!(log_mesh_file, m)?)?;
    m.add_function(wrap_pyfunction!(log_meshes, m)?)?;
    m.add_function(wrap_pyfunction!(log_pinhole, m)?)?;
    m.add_function(wrap_pyfunction!(log_rigid3, m)?)?;
    m.add_function(wrap_pyfunction!(log_unknown_transform, m)?)?;
    m.add_function(wrap_pyfunction!(log_view_coordinates_up_handedness, m)?)?;
    m.add_function(wrap_pyfunction!(log_view_coordinates_xyz, m)?)?;

    // misc
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(get_app_url, m)?)?;
    m.add_function(wrap_pyfunction!(start_web_viewer_server, m)?)?;

    Ok(())
}

// --- Init ---

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
        RecordingId::from_string(RecordingType::Data, recording_id)
    } else {
        default_recording_id(py, &application_id)
    };

    let recording = RecordingStreamBuilder::new(application_id)
        .is_official_example(is_official_example)
        .recording_id(recording_id.clone())
        .recording_source(re_log_types::RecordingSource::PythonSdk(python_version(py)))
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
        .recording_info()
        .map(|info| info.application_id.to_string())
}

#[pyfunction]
fn get_recording_id(recording: Option<&PyRecordingStream>) -> Option<String> {
    get_data_recording(recording)?
        .recording_info()
        .map(|info| info.recording_id.to_string())
}

/// Returns the currently active data recording in the global scope, if any; fallbacks to the
/// specified recording otherwise, if any.
#[pyfunction]
fn get_data_recording(recording: Option<&PyRecordingStream>) -> Option<PyRecordingStream> {
    RecordingStream::get(
        rerun::RecordingType::Data,
        recording.map(|rec| rec.0.clone()),
    )
    .map(PyRecordingStream)
}

/// Returns the currently active data recording in the global scope, if any.
#[pyfunction]
fn get_global_data_recording() -> Option<PyRecordingStream> {
    RecordingStream::global(rerun::RecordingType::Data).map(PyRecordingStream)
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
            rerun::RecordingType::Data,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream)
    })
}

/// Returns the currently active data recording in the thread-local scope, if any.
#[pyfunction]
fn get_thread_local_data_recording() -> Option<PyRecordingStream> {
    RecordingStream::thread_local(rerun::RecordingType::Data).map(PyRecordingStream)
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
            rerun::RecordingType::Data,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream)
    })
}

/// Returns the currently active blueprint recording in the global scope, if any; fallbacks to the
/// specified recording otherwise, if any.
#[pyfunction]
fn get_blueprint_recording(overrides: Option<&PyRecordingStream>) -> Option<PyRecordingStream> {
    RecordingStream::get(
        rerun::RecordingType::Blueprint,
        overrides.map(|rec| rec.0.clone()),
    )
    .map(PyRecordingStream)
}

/// Returns the currently active blueprint recording in the global scope, if any.
#[pyfunction]
fn get_global_blueprint_recording() -> Option<PyRecordingStream> {
    RecordingStream::global(rerun::RecordingType::Blueprint).map(PyRecordingStream)
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
            rerun::RecordingType::Blueprint,
            recording.map(|rec| rec.0.clone()),
        )
        .map(PyRecordingStream)
    })
}

/// Returns the currently active blueprint recording in the thread-local scope, if any.
#[pyfunction]
fn get_thread_local_blueprint_recording() -> Option<PyRecordingStream> {
    RecordingStream::thread_local(rerun::RecordingType::Blueprint).map(PyRecordingStream)
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
            rerun::RecordingType::Blueprint,
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
#[pyo3(signature = (addr = None, recording = None))]
fn connect(addr: Option<String>, recording: Option<&PyRecordingStream>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    let addr = if let Some(addr) = addr {
        addr.parse()?
    } else {
        rerun::default_server_addr()
    };

    recording.connect(addr);

    Ok(())
}

#[pyfunction]
#[pyo3(signature = (path, recording = None))]
fn save(path: &str, recording: Option<&PyRecordingStream>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

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
    /// This will do a blocking flush before returning!
    fn get_rrd_as_bytes<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
        // Release the GIL in case any flushing behavior needs to cleanup a python object.
        py.allow_threads(|| {
            self.rec.flush_blocking();
        });

        self.inner
            .rrd_as_bytes()
            .map(|bytes| PyBytes::new(py, bytes.as_slice()))
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
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
        let Some(recording) = get_data_recording(recording) else { return Ok(()); };

        let _guard = enter_tokio_runtime();

        recording.set_sink(
            rerun::web_viewer::new_sink(
                open_browser,
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
    let Some(recording) = get_data_recording(recording) else { return; };
    // Release the GIL in case any flushing behavior needs to cleanup a python object.
    py.allow_threads(|| {
        recording.disconnect();
    });
}

/// Block until outstanding data has been flushed to the sink
#[pyfunction]
fn flush(py: Python<'_>, blocking: bool, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else { return; };
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

fn time(timeless: bool, rec: &RecordingStream) -> TimePoint {
    if timeless {
        TimePoint::timeless()
    } else {
        rec.now()
    }
}

#[pyfunction]
fn set_time_sequence(timeline: &str, sequence: Option<i64>, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else { return; };
    recording.set_time_sequence(timeline, sequence);
}

#[pyfunction]
fn set_time_seconds(timeline: &str, seconds: Option<f64>, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else { return; };
    recording.set_time_seconds(timeline, seconds);
}

#[pyfunction]
fn set_time_nanos(timeline: &str, nanos: Option<i64>, recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else { return; };
    recording.set_time_nanos(timeline, nanos);
}

#[pyfunction]
fn reset_time(recording: Option<&PyRecordingStream>) {
    let Some(recording) = get_data_recording(recording) else { return; };
    recording.reset_time();
}

// --- Log transforms ---

#[pyfunction]
fn log_unknown_transform(
    entity_path: &str,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let transform = re_log_types::Transform::Unknown;
    log_transform(entity_path, transform, timeless, recording)
}

#[pyfunction]
fn log_rigid3(
    entity_path: &str,
    parent_from_child: bool,
    rotation_q: re_log_types::Quaternion,
    translation: [f32; 3],
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let rotation = glam::Quat::from_slice(&rotation_q);
    let translation = glam::Vec3::from_slice(&translation);
    let transform = macaw::IsoTransform::from_rotation_translation(rotation, translation);

    let transform = if parent_from_child {
        re_log_types::Rigid3::new_parent_from_child(transform)
    } else {
        re_log_types::Rigid3::new_child_from_parent(transform)
    };

    let transform = re_log_types::Transform::Rigid3(transform);

    log_transform(entity_path, transform, timeless, recording)
}

#[pyfunction]
fn log_pinhole(
    entity_path: &str,
    resolution: [f32; 2],
    child_from_parent: [[f32; 3]; 3],
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let transform = re_log_types::Transform::Pinhole(re_log_types::Pinhole {
        image_from_cam: child_from_parent.into(),
        resolution: Some(resolution.into()),
    });

    log_transform(entity_path, transform, timeless, recording)
}

fn log_transform(
    entity_path: &str,
    transform: re_log_types::Transform,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    let entity_path = parse_entity_path(entity_path)?;
    if entity_path.is_root() {
        return Err(PyTypeError::new_err("Transforms are between a child entity and its parent, so the root cannot have a transform"));
    }
    let time_point = time(timeless, &recording);

    // We currently log arrow transforms from inside the bridge because we are
    // using glam and macaw to potentially do matrix-inversion as part of the
    // logging pipeline. Implementing these data-transforms consistently on the
    // python side will take a bit of additional work and testing to ensure we aren't
    // introducing new numerical issues.

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        time_point,
        1,
        [transform].as_slice(),
    );

    recording.record_row(row);

    Ok(())
}

// --- Log view coordinates ---

#[pyfunction]
#[pyo3(signature = (entity_path, xyz, right_handed = None, timeless = false, recording=None))]
fn log_view_coordinates_xyz(
    entity_path: &str,
    xyz: &str,
    right_handed: Option<bool>,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let coordinates: ViewCoordinates = xyz.parse().map_err(PyTypeError::new_err)?;

    if let Some(right_handed) = right_handed {
        let expected_handedness = Handedness::from_right_handed(right_handed);
        let actual_handedness = coordinates.handedness().unwrap(); // can't fail if we managed to parse

        if actual_handedness != expected_handedness {
            return Err(PyTypeError::new_err(format!(
                "Mismatched handedness. {} is {}",
                coordinates.describe(),
                actual_handedness.describe(),
            )));
        }
    }

    log_view_coordinates(entity_path, coordinates, timeless, recording)
}

#[pyfunction]
fn log_view_coordinates_up_handedness(
    entity_path: &str,
    up: &str,
    right_handed: bool,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let up = up.parse::<SignedAxis3>().map_err(PyTypeError::new_err)?;
    let handedness = Handedness::from_right_handed(right_handed);
    let coordinates = ViewCoordinates::from_up_and_handedness(up, handedness);

    log_view_coordinates(entity_path, coordinates, timeless, recording)
}

fn log_view_coordinates(
    entity_path_str: &str,
    coordinates: ViewCoordinates,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    if coordinates.handedness() == Some(Handedness::Left) {
        re_log::warn_once!("Left-handed coordinate systems are not yet fully supported by Rerun");
    }

    // We normally disallow logging to root, but we make an exception for view_coordinates
    let entity_path = if entity_path_str == "/" {
        EntityPath::root()
    } else {
        parse_entity_path(entity_path_str)?
    };

    let time_point = time(timeless, &recording);

    // We currently log view coordinates from inside the bridge because the code
    // that does matching and validation on different string representations is
    // non-trivial. Implementing this functionality on the python side will take
    // a bit of additional work and testing to ensure we aren't introducing new
    // conversion errors.

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        time_point,
        1,
        [coordinates].as_slice(),
    );

    recording.record_row(row);

    Ok(())
}

// --- Log segmentation ---

#[derive(FromPyObject)]
struct AnnotationInfoTuple(u16, Option<String>, Option<Vec<u8>>);

impl From<AnnotationInfoTuple> for AnnotationInfo {
    fn from(tuple: AnnotationInfoTuple) -> Self {
        let AnnotationInfoTuple(id, label, color) = tuple;
        Self {
            id,
            label: label.map(Label),
            color: color
                .as_ref()
                .map(|color| convert_color(color.clone()).unwrap())
                .map(|bytes| bytes.into()),
        }
    }
}

type ClassDescriptionTuple = (AnnotationInfoTuple, Vec<AnnotationInfoTuple>, Vec<u16>);

#[pyfunction]
fn log_annotation_context(
    entity_path_str: &str,
    class_descriptions: Vec<ClassDescriptionTuple>,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    // We normally disallow logging to root, but we make an exception for class_descriptions
    let entity_path = if entity_path_str == "/" {
        EntityPath::root()
    } else {
        parse_entity_path(entity_path_str)?
    };

    let mut annotation_context = AnnotationContext::default();

    for (info, keypoint_annotations, keypoint_skeleton_edges) in class_descriptions {
        annotation_context
            .class_map
            .entry(ClassId(info.0))
            .or_insert_with(|| ClassDescription {
                info: info.into(),
                keypoint_map: keypoint_annotations
                    .into_iter()
                    .map(|k| (KeypointId(k.0), k.into()))
                    .collect(),
                keypoint_connections: keypoint_skeleton_edges
                    .chunks_exact(2)
                    .map(|pair| (KeypointId(pair[0]), KeypointId(pair[1])))
                    .collect(),
            });
    }

    let time_point = time(timeless, &recording);

    // We currently log AnnotationContext from inside the bridge because it's a
    // fairly complex type with a need for a fair amount of data-validation. We
    // already have the serialization implemented in rust so we start with this
    // implementation.
    //
    // TODO(jleibs) replace with python-native implementation

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        time_point,
        1,
        [annotation_context].as_slice(),
    );

    recording.record_row(row);

    Ok(())
}

// --- Log assets ---

#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn log_meshes(
    entity_path_str: &str,
    position_buffers: Vec<numpy::PyReadonlyArray1<'_, f32>>,
    vertex_color_buffers: Vec<Option<numpy::PyReadonlyArray2<'_, u8>>>,
    index_buffers: Vec<Option<numpy::PyReadonlyArray1<'_, u32>>>,
    normal_buffers: Vec<Option<numpy::PyReadonlyArray1<'_, f32>>>,
    albedo_factors: Vec<Option<numpy::PyReadonlyArray1<'_, f32>>>,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    let entity_path = parse_entity_path(entity_path_str)?;

    // Make sure we have as many position buffers as index buffers, etc.
    if position_buffers.len() != vertex_color_buffers.len()
        || position_buffers.len() != index_buffers.len()
        || position_buffers.len() != normal_buffers.len()
        || position_buffers.len() != albedo_factors.len()
    {
        return Err(PyTypeError::new_err(format!(
            "Top-level position/index/normal/albedo buffer arrays must be same the length, \
                got positions={}, vertex_colors={}, indices={}, normals={}, albedo={} instead",
            position_buffers.len(),
            vertex_color_buffers.len(),
            index_buffers.len(),
            normal_buffers.len(),
            albedo_factors.len(),
        )));
    }

    let time_point = time(timeless, &recording);

    let mut meshes = Vec::with_capacity(position_buffers.len());

    for (vertex_positions, vertex_colors, indices, normals, albedo_factor) in izip!(
        position_buffers,
        vertex_color_buffers,
        index_buffers,
        normal_buffers,
        albedo_factors,
    ) {
        let albedo_factor =
            if let Some(v) = albedo_factor.map(|albedo_factor| albedo_factor.as_array().to_vec()) {
                match v.len() {
                    3 => Vec4D([v[0], v[1], v[2], 1.0]),
                    4 => Vec4D([v[0], v[1], v[2], v[3]]),
                    _ => {
                        return Err(PyTypeError::new_err(format!(
                            "Albedo factor must be vec3 or vec4, got {v:?} instead",
                        )));
                    }
                }
                .into()
            } else {
                None
            };

        let vertex_colors = if let Some(vertex_colors) = vertex_colors {
            match vertex_colors.shape() {
                [_, 3] => Some(
                    slice_from_np_array(&vertex_colors)
                        .chunks_exact(3)
                        .map(|c| ColorRGBA::from_rgb(c[0], c[1], c[2]).0)
                        .collect(),
                ),
                [_, 4] => Some(
                    slice_from_np_array(&vertex_colors)
                        .chunks_exact(4)
                        .map(|c| ColorRGBA::from_unmultiplied_rgba(c[0], c[1], c[2], c[3]).0)
                        .collect(),
                ),
                shape => {
                    return Err(PyTypeError::new_err(format!(
                        "Expected vertex colors to have a Nx3 or Nx4 shape, got {shape:?} instead",
                    )));
                }
            }
        } else {
            None
        };

        let raw = RawMesh3D {
            mesh_id: MeshId::random(),
            vertex_positions: vertex_positions.as_array().to_vec().into(),
            vertex_colors,
            indices: indices.map(|indices| indices.as_array().to_vec().into()),
            vertex_normals: normals.map(|normals| normals.as_array().to_vec().into()),
            albedo_factor,
        };
        raw.sanity_check()
            .map_err(|err| PyTypeError::new_err(err.to_string()))?;

        meshes.push(Mesh3D::Raw(raw));
    }

    // We currently log `Mesh3D` from inside the bridge.
    //
    // Pyarrow handling of nested unions was causing more grief that it was
    // worth fighting with in the short term.
    //
    // TODO(jleibs) replace with python-native implementation

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        time_point,
        meshes.len() as _,
        meshes,
    );

    recording.record_row(row);

    Ok(())
}

#[pyfunction]
fn log_mesh_file(
    entity_path_str: &str,
    mesh_format: &str,
    transform: numpy::PyReadonlyArray2<'_, f32>,
    timeless: bool,
    mesh_bytes: Option<Vec<u8>>,
    mesh_path: Option<PathBuf>,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    let entity_path = parse_entity_path(entity_path_str)?;

    let format = match mesh_format {
        "GLB" => MeshFormat::Glb,
        "GLTF" => MeshFormat::Gltf,
        "OBJ" => MeshFormat::Obj,
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Unknown mesh format {mesh_format:?}. \
                Expected one of: GLB, GLTF, OBJ"
            )));
        }
    };

    let mesh_bytes = match (mesh_bytes, mesh_path) {
        (Some(mesh_bytes), None) => mesh_bytes,
        (None, Some(mesh_path)) => std::fs::read(mesh_path)?,
        (None, None) => Err(PyTypeError::new_err(
            "log_mesh_file: You must pass either mesh_bytes or mesh_path",
        ))?,
        (Some(_), Some(_)) => Err(PyTypeError::new_err(
            "log_mesh_file: You must pass either mesh_bytes or mesh_path, but not both!",
        ))?,
    };

    let transform = if transform.is_empty() {
        [
            [1.0, 0.0, 0.0], // col 0
            [0.0, 1.0, 0.0], // col 1
            [0.0, 0.0, 1.0], // col 2
            [0.0, 0.0, 0.0], // col 3 = translation
        ]
    } else {
        if transform.shape() != [3, 4] {
            return Err(PyTypeError::new_err(format!(
                "Expected a 3x4 affine transformation matrix, got shape={:?}",
                transform.shape()
            )));
        }

        let get = |row, col| *transform.get([row, col]).unwrap();

        [
            [get(0, 0), get(1, 0), get(2, 0)], // col 0
            [get(0, 1), get(1, 1), get(2, 1)], // col 1
            [get(0, 2), get(1, 2), get(2, 2)], // col 2
            [get(0, 3), get(1, 3), get(2, 3)], // col 3 = translation
        ]
    };

    let time_point = time(timeless, &recording);

    let mesh3d = Mesh3D::Encoded(EncodedMesh3D {
        mesh_id: MeshId::random(),
        format,
        bytes: mesh_bytes.into(),
        transform,
    });

    // We currently log `Mesh3D` from inside the bridge.
    //
    // Pyarrow handling of nested unions was causing more grief that it was
    // worth fighting with in the short term.
    //
    // TODO(jleibs) replace with python-native implementation

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        time_point,
        1,
        [mesh3d].as_slice(),
    );

    recording.record_row(row);

    Ok(())
}

/// Log an image file given its contents or path on disk.
///
/// If no `img_format` is specified, we will try and guess it.
#[pyfunction]
#[pyo3(signature = (entity_path, img_bytes = None, img_path = None, img_format = None, timeless = false, recording=None))]
fn log_image_file(
    entity_path: &str,
    img_bytes: Option<Vec<u8>>,
    img_path: Option<PathBuf>,
    img_format: Option<&str>,
    timeless: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    let entity_path = parse_entity_path(entity_path)?;

    let img_bytes = match (img_bytes, img_path) {
        (Some(img_bytes), None) => img_bytes,
        (None, Some(img_path)) => std::fs::read(img_path)?,
        (None, None) => Err(PyTypeError::new_err(
            "log_image_file: You must pass either img_bytes or img_path",
        ))?,
        (Some(_), Some(_)) => Err(PyTypeError::new_err(
            "log_image_file: You must pass either img_bytes or img_path, but not both!",
        ))?,
    };

    let img_format = match img_format {
        Some(img_format) => image::ImageFormat::from_extension(img_format)
            .ok_or_else(|| PyTypeError::new_err(format!("Unknown image format {img_format:?}.")))?,
        None => {
            image::guess_format(&img_bytes).map_err(|err| PyTypeError::new_err(err.to_string()))?
        }
    };

    let tensor = Tensor::from_image_bytes(img_bytes, img_format)
        .map_err(|err| PyTypeError::new_err(err.to_string()))?;

    let time_point = time(timeless, &recording);

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        time_point,
        1,
        [tensor].as_slice(),
    );

    recording.record_row(row);

    Ok(())
}

// TODO(jleibs): This shadows [`re_log_types::TensorDataMeaning`]
#[pyclass]
#[derive(Clone, Debug)]
enum TensorDataMeaning {
    Unknown,
    ClassId,
    Depth,
}

// --- Log special ---

#[pyfunction]
fn log_cleared(
    entity_path: &str,
    recursive: bool,
    recording: Option<&PyRecordingStream>,
) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    let entity_path = parse_entity_path(entity_path)?;
    let timepoint = time(false, &recording);

    recording.record_path_op(timepoint, PathOp::clear(recursive, entity_path));

    Ok(())
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
    let Some(recording) = get_data_recording(recording) else { return Ok(()); };

    let entity_path = parse_entity_path(entity_path)?;
    let timepoint = time(timeless, &recording);

    // It's important that we don't hold the session lock while building our arrow component.
    // the API we call to back through pyarrow temporarily releases the GIL, which can cause
    // cause a deadlock.
    let row = crate::arrow::build_data_row_from_components(&entity_path, components, &timepoint)?;

    recording.record_row(row);

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
        return format!("http://localhost:{}", hosted_assets.port());
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
            re_web_viewer_server::WebViewerServerHandle::new(WebViewerServerPort(port)).map_err(
                |err| {
                    PyRuntimeError::new_err(format!(
                        "Failed to start web viewer server on port {port}: {err}",
                    ))
                },
            )?,
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

fn default_recording_id(py: Python<'_>, application_id: &str) -> RecordingId {
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
    let seed = authkey(py);
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
    RecordingId::from_uuid(RecordingType::Data, uuid)
}

fn authkey(py: Python<'_>) -> Vec<u8> {
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
    .unwrap();
    let authkey = locals.get_item("authkey").unwrap();
    let authkey: &PyBytes = authkey.downcast().unwrap();
    authkey.as_bytes().to_vec()
}

fn convert_color(color: Vec<u8>) -> PyResult<[u8; 4]> {
    match &color[..] {
        [r, g, b] => Ok([*r, *g, *b, 255]),
        [r, g, b, a] => Ok([*r, *g, *b, *a]),
        _ => Err(PyTypeError::new_err(format!(
            "Expected color to be of length 3 or 4, got {color:?}"
        ))),
    }
}

fn slice_from_np_array<'a, T: numpy::Element, D: numpy::ndarray::Dimension>(
    array: &'a numpy::PyReadonlyArray<'_, T, D>,
) -> Cow<'a, [T]> {
    let array = array.as_array();

    // Numpy has many different memory orderings.
    // We could/should check that we have the right one here.
    // But for now, we just check for and optimize the trivial case.
    if array.shape().len() == 1 {
        if let Some(slice) = array.to_slice() {
            return Cow::Borrowed(slice); // common-case optimization
        }
    }

    Cow::Owned(array.iter().cloned().collect())
}

fn parse_entity_path(entity_path: &str) -> PyResult<EntityPath> {
    let components = re_log_types::parse_entity_path(entity_path)
        .map_err(|err| PyTypeError::new_err(err.to_string()))?;
    if components.is_empty() {
        Err(PyTypeError::new_err(
            "You cannot log to the root {entity_path:?}",
        ))
    } else {
        Ok(EntityPath::from(components))
    }
}
