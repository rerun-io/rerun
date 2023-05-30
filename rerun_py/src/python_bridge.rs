#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use std::{borrow::Cow, io::Cursor, path::PathBuf};

use itertools::izip;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyDict},
};

use depthai_viewer::{
    log::{PathOp, RowId},
    sink::MemorySinkStorage,
    time::{Time, TimeInt, TimePoint, TimeType, Timeline},
    ApplicationId, EntityPath, RecordingId,
};
use re_log_types::{ArrowMsg, DataRow, DataTableError};

pub use depthai_viewer::{
    components::{
        AnnotationContext, AnnotationInfo, Arrow3D, Box3D, ClassDescription, ClassId, ColorRGBA,
        EncodedMesh3D, InstanceKey, KeypointId, Label, LineStrip2D, LineStrip3D, Mat3x3, Mesh3D,
        MeshFormat, MeshId, Pinhole, Point2D, Point3D, Quaternion, Radius, RawMesh3D, Rect2D,
        Rigid3, Scalar, ScalarPlotProps, Size3D, Tensor, TensorData, TensorDimension, TensorId,
        TextEntry, Transform, Vec2D, Vec3D, Vec4D, ViewCoordinates,
    },
    coordinates::{Axis3, Handedness, Sign, SignedAxis3},
};

#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "web_viewer")]
use re_ws_comms::RerunServerPort;

use crate::{arrow::get_registered_component_names, python_session::PythonSession};

// ----------------------------------------------------------------------------

/// The global [`PythonSession`] object used by the Python API.
fn python_session() -> parking_lot::MutexGuard<'static, PythonSession> {
    use once_cell::sync::OnceCell;
    use parking_lot::Mutex;
    static PYTHON_SESSION: OnceCell<Mutex<PythonSession>> = OnceCell::new();
    PYTHON_SESSION.get_or_init(Default::default).lock()
}

// ----------------------------------------------------------------------------

/// Thread-local info
#[derive(Default)]
struct ThreadInfo {
    /// The current time, which can be set by users.
    time_point: TimePoint,
}

impl ThreadInfo {
    pub fn thread_now() -> TimePoint {
        Self::with(|ti| ti.now())
    }

    pub fn set_thread_time(timeline: Timeline, time_int: Option<TimeInt>) {
        Self::with(|ti| ti.set_time(timeline, time_int));
    }

    pub fn reset_thread_time() {
        Self::with(|ti| ti.reset_time());
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

    fn now(&self) -> TimePoint {
        let mut time_point = self.time_point.clone();
        time_point.insert(Timeline::log_time(), Time::now().into());
        time_point
    }

    fn set_time(&mut self, timeline: Timeline, time_int: Option<TimeInt>) {
        if let Some(time_int) = time_int {
            self.time_point.insert(timeline, time_int);
        } else {
            self.time_point.remove(&timeline);
        }
    }

    fn reset_time(&mut self) {
        self.time_point = TimePoint::default();
    }
}

// ----------------------------------------------------------------------------

fn python_version(py: Python<'_>) -> re_log_types::PythonVersion {
    let py_version = py.version_info();
    re_log_types::PythonVersion {
        major: py_version.major,
        minor: py_version.minor,
        patch: py_version.patch,
        suffix: py_version.suffix.map(|s| s.to_owned()).unwrap_or_default(),
    }
}

/// The python module is called "depthai_viewer_bindings".
#[pymodule]
fn depthai_viewer_bindings(py: Python<'_>, m: &PyModule) -> PyResult<()> {
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

    // If this is a special RERUN_APP_ONLY context (launched via .spawn), we
    // can bypass everything else, which keeps us from preparing an SDK session
    // that never gets used.
    if matches!(std::env::var("RERUN_APP_ONLY").as_deref(), Ok("true")) {
        return Ok(());
    }
    let sys_exe: String = py.import("sys")?.getattr("executable")?.extract()?;
    python_session().set_python_version(python_version(py), sys_exe);

    // NOTE: We do this here because we want child processes to share the same recording-id,
    // whether the user has called `init` or not.
    // See `default_recording_id` for extra information.
    python_session().set_recording_id(default_recording_id(py));

    m.add_function(wrap_pyfunction!(get_recording_id, m)?)?;
    m.add_function(wrap_pyfunction!(set_recording_id, m)?)?;

    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add_function(wrap_pyfunction!(disconnect, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;
    m.add_function(wrap_pyfunction!(get_app_url, m)?)?;
    m.add_function(wrap_pyfunction!(init, m)?)?;
    m.add_function(wrap_pyfunction!(is_enabled, m)?)?;
    m.add_function(wrap_pyfunction!(memory_recording, m)?)?;
    m.add_function(wrap_pyfunction!(save, m)?)?;
    m.add_function(wrap_pyfunction!(start_web_viewer_server, m)?)?;
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    m.add_function(wrap_pyfunction!(set_enabled, m)?)?;
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;

    m.add_function(wrap_pyfunction!(set_time_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_nanos, m)?)?;
    m.add_function(wrap_pyfunction!(reset_time, m)?)?;

    m.add_function(wrap_pyfunction!(log_unknown_transform, m)?)?;
    m.add_function(wrap_pyfunction!(log_rigid3, m)?)?;
    m.add_function(wrap_pyfunction!(log_pinhole, m)?)?;

    m.add_function(wrap_pyfunction!(log_meshes, m)?)?;

    m.add_function(wrap_pyfunction!(log_view_coordinates_xyz, m)?)?;
    m.add_function(wrap_pyfunction!(log_view_coordinates_up_handedness, m)?)?;

    m.add_function(wrap_pyfunction!(log_annotation_context, m)?)?;

    m.add_function(wrap_pyfunction!(log_mesh_file, m)?)?;
    m.add_function(wrap_pyfunction!(log_image_file, m)?)?;
    m.add_function(wrap_pyfunction!(log_cleared, m)?)?;
    m.add_function(wrap_pyfunction!(log_arrow_msg, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;

    Ok(())
}

fn default_recording_id(py: Python<'_>) -> RecordingId {
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
    let mut rng = rand::rngs::StdRng::seed_from_u64(hasher.finish());
    let uuid = uuid::Builder::from_random_bytes(rng.gen()).into_uuid();
    RecordingId::from_uuid(uuid)
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

// ----------------------------------------------------------------------------

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

fn time(timeless: bool) -> TimePoint {
    if timeless {
        TimePoint::timeless()
    } else {
        ThreadInfo::thread_now()
    }
}

// ----------------------------------------------------------------------------

#[pyfunction]
fn main(py: Python<'_>, argv: Vec<String>, sys_exe: String, venv_site: String) -> PyResult<u8> {
    let build_info = re_build_info::build_info!();
    let call_src = depthai_viewer::CallSource::Python(python_version(py), sys_exe, venv_site);
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(depthai_viewer::run(build_info, call_src, argv))
        .map_err(|err| PyRuntimeError::new_err(re_error::format(err)))
}

#[pyfunction]
fn get_recording_id() -> PyResult<String> {
    let recording_id = python_session().recording_id();

    if recording_id == RecordingId::ZERO {
        Err(PyTypeError::new_err("module has not been initialized"))
    } else {
        Ok(recording_id.to_string())
    }
}

#[pyfunction]
fn set_recording_id(recording_id: &str) -> PyResult<()> {
    if let Ok(recording_id) = recording_id.parse() {
        python_session().set_recording_id(recording_id);
        Ok(())
    } else {
        Err(PyTypeError::new_err(format!(
            "Invalid recording id - expected a UUID, got {recording_id:?}"
        )))
    }
}

#[pyfunction]
#[pyo3(signature = (application_id, application_path=None, default_enabled=true))]
fn init(application_id: String, application_path: Option<PathBuf>, default_enabled: bool) {
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

    let mut session = python_session();
    session.set_default_enabled(default_enabled);
    session.set_application_id(ApplicationId(application_id), is_official_example);
}

#[pyfunction]
fn connect(addr: Option<String>) -> PyResult<()> {
    let addr = if let Some(addr) = addr {
        addr.parse()?
    } else {
        depthai_viewer::default_server_addr()
    };
    python_session().connect(addr);
    Ok(())
}

#[pyfunction]
fn version() -> PyResult<String> {
    Ok(python_session().version())
}

#[must_use = "the tokio_runtime guard must be kept alive while using tokio"]
#[cfg(feature = "web_viewer")]
fn enter_tokio_runtime() -> tokio::runtime::EnterGuard<'static> {
    use once_cell::sync::Lazy;
    static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
        Lazy::new(|| tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"));
    TOKIO_RUNTIME.enter()
}

/// Serve a web-viewer.
#[allow(clippy::unnecessary_wraps)] // False positive
#[pyfunction]
fn serve(open_browser: bool, web_port: Option<u16>, ws_port: Option<u16>) -> PyResult<()> {
    #[cfg(feature = "web_viewer")]
    {
        let mut session = python_session();

        if !session.is_enabled() {
            re_log::debug!("Rerun disabled - call to serve() ignored");
            return Ok(());
        }

        let _guard = enter_tokio_runtime();

        session.set_sink(
            depthai_viewer::web_viewer::new_sink(
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
        _ = web_port;
        _ = ws_port;
        _ = open_browser;
        Err(PyRuntimeError::new_err(
            "The Rerun SDK was not compiled with the 'web_viewer' feature",
        ))
    }
}

#[pyfunction]
// TODO(jleibs) expose this as a python type
fn start_web_viewer_server(port: u16) -> PyResult<()> {
    #[cfg(feature = "web_viewer")]
    {
        let mut session = python_session();
        let _guard = enter_tokio_runtime();
        session
            .start_web_viewer_server(WebViewerServerPort(port))
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
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
fn get_app_url() -> String {
    let session = python_session();
    session.get_app_url()
}

#[pyfunction]
fn shutdown(py: Python<'_>) {
    re_log::debug!("Shutting down the Rerun SDK");
    // Disconnect the current sink which ensures that
    // it flushes and cleans up.
    disconnect(py);
}

/// Is logging enabled in the global session?
#[pyfunction]
fn is_enabled() -> bool {
    python_session().is_enabled()
}

/// Enable or disable logging in the global session.
///
/// This is a global setting that affects all threads.
/// By default logging is enabled.
#[pyfunction]
fn set_enabled(enabled: bool) {
    python_session().set_enabled(enabled);
}

/// Block until outstanding data has been flushed to the sink
#[pyfunction]
fn flush(py: Python<'_>) {
    // Release the GIL in case any flushing behavior needs to
    // cleanup a python object.
    py.allow_threads(|| {
        python_session().flush();
    });
}

/// Disconnect from remote server (if any).
///
/// Subsequent log messages will be buffered and either sent on the next call to `connect`,
/// or shown with `show`.
#[pyfunction]
fn disconnect(py: Python<'_>) {
    // Release the GIL in case any flushing behavior needs to
    // cleanup a python object.
    py.allow_threads(|| {
        python_session().disconnect();
    });
}

#[pyfunction]
fn save(path: &str) -> PyResult<()> {
    let mut session = python_session();
    session
        .save(path)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

#[pyclass]
struct PyMemorySinkStorage(MemorySinkStorage);

#[pymethods]
impl PyMemorySinkStorage {
    fn get_rrd_as_bytes<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
        self.0
            .rrd_as_bytes()
            .map(|bytes| PyBytes::new(py, bytes.as_slice()))
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }
}

/// Create an in-memory rrd file
#[pyfunction]
fn memory_recording() -> PyMemorySinkStorage {
    let mut session = python_session();
    PyMemorySinkStorage(session.memory_recording())
}

// ----------------------------------------------------------------------------

/// Set the current time globally. Used for all subsequent logging,
/// until the next call to `set_time_sequence`.
///
/// For example: `set_time_sequence("frame_nr", frame_nr)`.
///
/// You can remove a timeline again using `set_time_sequence("frame_nr", None)`.
#[pyfunction]
fn set_time_sequence(timeline: &str, sequence: Option<i64>) {
    ThreadInfo::set_thread_time(
        Timeline::new(timeline, TimeType::Sequence),
        sequence.map(TimeInt::from),
    );
}

#[pyfunction]
fn set_time_seconds(timeline: &str, seconds: Option<f64>) {
    ThreadInfo::set_thread_time(
        Timeline::new(timeline, TimeType::Time),
        seconds.map(|secs| Time::from_seconds_since_epoch(secs).into()),
    );
}

#[pyfunction]
fn set_time_nanos(timeline: &str, ns: Option<i64>) {
    ThreadInfo::set_thread_time(
        Timeline::new(timeline, TimeType::Time),
        ns.map(|ns| Time::from_ns_since_epoch(ns).into()),
    );
}

#[pyfunction]
fn reset_time() {
    ThreadInfo::reset_thread_time();
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

#[pyfunction]
fn log_unknown_transform(entity_path: &str, timeless: bool) -> PyResult<()> {
    let transform = re_log_types::Transform::Unknown;
    log_transform(entity_path, transform, timeless)
}

#[pyfunction]
fn log_rigid3(
    entity_path: &str,
    parent_from_child: bool,
    rotation_q: re_log_types::Quaternion,
    translation: [f32; 3],
    timeless: bool,
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

    log_transform(entity_path, transform, timeless)
}

#[pyfunction]
fn log_pinhole(
    entity_path: &str,
    resolution: [f32; 2],
    child_from_parent: [[f32; 3]; 3],
    timeless: bool,
) -> PyResult<()> {
    let transform = re_log_types::Transform::Pinhole(re_log_types::Pinhole {
        image_from_cam: child_from_parent.into(),
        resolution: Some(resolution.into()),
    });

    log_transform(entity_path, transform, timeless)
}

fn log_transform(
    entity_path: &str,
    transform: re_log_types::Transform,
    timeless: bool,
) -> PyResult<()> {
    let entity_path = parse_entity_path(entity_path)?;
    if entity_path.is_root() {
        return Err(PyTypeError::new_err("Transforms are between a child entity and its parent, so the root cannot have a transform"));
    }
    let mut session = python_session();
    let time_point = time(timeless);

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

    session.send_row(row)
}

// ----------------------------------------------------------------------------

#[pyfunction]
#[pyo3(signature = (entity_path, xyz, right_handed = None, timeless = false))]
fn log_view_coordinates_xyz(
    entity_path: &str,
    xyz: &str,
    right_handed: Option<bool>,
    timeless: bool,
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

    log_view_coordinates(entity_path, coordinates, timeless)
}

#[pyfunction]
fn log_view_coordinates_up_handedness(
    entity_path: &str,
    up: &str,
    right_handed: bool,
    timeless: bool,
) -> PyResult<()> {
    let up = up.parse::<SignedAxis3>().map_err(PyTypeError::new_err)?;
    let handedness = Handedness::from_right_handed(right_handed);
    let coordinates = ViewCoordinates::from_up_and_handedness(up, handedness);

    log_view_coordinates(entity_path, coordinates, timeless)
}

fn log_view_coordinates(
    entity_path_str: &str,
    coordinates: ViewCoordinates,
    timeless: bool,
) -> PyResult<()> {
    if coordinates.handedness() == Some(Handedness::Left) {
        re_log::warn_once!("Left-handed coordinate systems are not yet fully supported by Rerun");
    }

    // We normally disallow logging to root, but we make an exception for view_coordinates
    let entity_path = if entity_path_str == "/" {
        EntityPath::root()
    } else {
        parse_entity_path(entity_path_str)?
    };

    let mut session = python_session();
    let time_point = time(timeless);

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

    session.send_row(row)
}

// ----------------------------------------------------------------------------

// TODO(jleibs): This shadows [`re_log_types::TensorDataMeaning`]
//
#[pyclass]
#[derive(Clone, Debug)]
enum TensorDataMeaning {
    Unknown,
    ClassId,
    Depth,
}

// ----------------------------------------------------------------------------

#[pyfunction]
fn log_meshes(
    entity_path_str: &str,
    position_buffers: Vec<numpy::PyReadonlyArray1<'_, f32>>,
    vertex_color_buffers: Vec<Option<numpy::PyReadonlyArray2<'_, u8>>>,
    index_buffers: Vec<Option<numpy::PyReadonlyArray1<'_, u32>>>,
    normal_buffers: Vec<Option<numpy::PyReadonlyArray1<'_, f32>>>,
    albedo_factors: Vec<Option<numpy::PyReadonlyArray1<'_, f32>>>,
    timeless: bool,
) -> PyResult<()> {
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

    let mut session = python_session();

    let time_point = time(timeless);

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

    session.send_row(row)
}

#[pyfunction]
fn log_mesh_file(
    entity_path_str: &str,
    mesh_format: &str,
    bytes: &[u8],
    transform: numpy::PyReadonlyArray2<'_, f32>,
    timeless: bool,
) -> PyResult<()> {
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
    let bytes: Vec<u8> = bytes.into();
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

    let mut session = python_session();

    let time_point = time(timeless);

    let mesh3d = Mesh3D::Encoded(EncodedMesh3D {
        mesh_id: MeshId::random(),
        format,
        bytes: bytes.into(),
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

    session.send_row(row)
}

/// Log an image file given its contents or path on disk.
///
/// If no `img_format` is specified, we will try and guess it.
#[pyfunction]
#[pyo3(signature = (entity_path, img_bytes = None, img_path = None, img_format = None, timeless = false))]
fn log_image_file(
    entity_path: &str,
    img_bytes: Option<Vec<u8>>,
    img_path: Option<PathBuf>,
    img_format: Option<&str>,
    timeless: bool,
) -> PyResult<()> {
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

    use image::ImageDecoder as _;
    let (w, h) = match img_format {
        image::ImageFormat::Jpeg => {
            use image::codecs::jpeg::JpegDecoder;
            let jpeg = JpegDecoder::new(Cursor::new(&img_bytes))
                .map_err(|err| PyTypeError::new_err(err.to_string()))?;

            let color_format = jpeg.color_type();
            if !matches!(color_format, image::ColorType::Rgb8) {
                // TODO(emilk): support gray-scale jpeg aswell
                return Err(PyTypeError::new_err(format!(
                    "Unsupported color format {color_format:?}. \
                    Expected one of: RGB8"
                )));
            }

            jpeg.dimensions()
        }
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Unsupported image format {img_format:?}. \
                Expected one of: JPEG"
            )))
        }
    };

    let mut session = python_session();

    let time_point = time(timeless);

    let tensor = re_log_types::component_types::Tensor {
        tensor_id: TensorId::random(),
        shape: vec![
            TensorDimension::height(h as _),
            TensorDimension::width(w as _),
            TensorDimension::depth(3),
        ],
        data: re_log_types::component_types::TensorData::JPEG(img_bytes.into()),
        meaning: re_log_types::component_types::TensorDataMeaning::Unknown,
        meter: None,
    };

    let row = DataRow::from_cells1(
        RowId::random(),
        entity_path,
        time_point,
        1,
        [tensor].as_slice(),
    );

    session.send_row(row)
}

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
) -> PyResult<()> {
    let mut session = python_session();

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

    let time_point = time(timeless);

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

    session.send_row(row)
}

#[pyfunction]
fn log_cleared(entity_path: &str, recursive: bool) -> PyResult<()> {
    let entity_path = parse_entity_path(entity_path)?;
    let mut session = python_session();

    let time_point = time(false);

    session.send_path_op(&time_point, PathOp::clear(recursive, entity_path));

    Ok(())
}

#[pyfunction]
fn log_arrow_msg(entity_path: &str, components: &PyDict, timeless: bool) -> PyResult<()> {
    let entity_path = parse_entity_path(entity_path)?;

    // It's important that we don't hold the session lock while building our arrow component.
    // the API we call to back through pyarrow temporarily releases the GIL, which can cause
    // cause a deadlock.
    let data_table =
        crate::arrow::build_data_table_from_components(&entity_path, components, &time(timeless))?;

    let mut session = python_session();

    let msg: ArrowMsg = data_table
        .to_arrow_msg()
        .map_err(|err: DataTableError| PyValueError::new_err(err.to_string()))?;

    session.send_arrow_msg(msg);

    Ok(())
}

// ----------------------------------------------------------------------------

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
