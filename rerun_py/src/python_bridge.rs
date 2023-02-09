#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use std::{io::Cursor, path::PathBuf};

use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::PyDict,
};

// init
pub use rerun::{global_session, ApplicationId, RecordingId};

// time
use rerun::{Time, TimeInt, TimePoint, TimeType, Timeline};

// messages
use rerun::{EntityPath, LogMsg, MsgBundle, MsgId, PathOp};

// components
pub use rerun::{
    AnnotationContext, AnnotationInfo, Arrow3D, Axis3, Box3D, ClassDescription, ClassId, ColorRGBA,
    EncodedMesh3D, Handedness, InstanceKey, KeypointId, Label, LineStrip2D, LineStrip3D, Mat3x3,
    Mesh3D, MeshFormat, MeshId, Pinhole, Point2D, Point3D, Quaternion, Radius, RawMesh3D, Rect2D,
    Rigid3, Scalar, ScalarPlotProps, Sign, SignedAxis3, Size3D, Tensor, TensorData,
    TensorDimension, TensorId, TensorTrait, TextEntry, Transform, Vec2D, Vec3D, Vec4D,
    ViewCoordinates,
};

use crate::arrow::get_registered_component_names;

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
}

// ----------------------------------------------------------------------------

/// The python module is called "rerun_bindings".
#[pymodule]
fn rerun_bindings(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // NOTE: We do this here because some the inner init methods don't respond too kindly to being
    // called more than once.
    re_log::setup_native_logging();

    // NOTE: We do this here because we want child processes to share the same recording-id,
    // whether the user has called `init` or not.
    // See `default_recording_id` for extra information.
    global_session().set_recording_id(default_recording_id(py));

    m.add_function(wrap_pyfunction!(main, m)?)?;

    m.add_function(wrap_pyfunction!(get_registered_component_names, m)?)?;

    m.add_function(wrap_pyfunction!(get_recording_id, m)?)?;
    m.add_function(wrap_pyfunction!(set_recording_id, m)?)?;

    m.add_function(wrap_pyfunction!(init, m)?)?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;

    #[cfg(feature = "re_viewer")]
    {
        m.add_function(wrap_pyfunction!(disconnect, m)?)?;
        m.add_function(wrap_pyfunction!(show, m)?)?;
    }
    m.add_function(wrap_pyfunction!(save, m)?)?;

    m.add_function(wrap_pyfunction!(set_time_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_nanos, m)?)?;

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

    m.add_class::<TensorDataMeaning>()?;

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
    use pyo3::types::PyBytes;
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
fn main(argv: Vec<String>) -> PyResult<u8> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(rerun::run(rerun::CallSource::Python, argv))
        .map_err(|err| PyRuntimeError::new_err(re_error::format(err)))
}

#[pyfunction]
fn get_recording_id() -> PyResult<String> {
    global_session()
        .recording_id()
        .ok_or_else(|| PyTypeError::new_err("module has not been initialized"))
        .map(|recording_id| recording_id.to_string())
}

#[pyfunction]
fn set_recording_id(recording_id: &str) -> PyResult<()> {
    if let Ok(recording_id) = recording_id.parse() {
        global_session().set_recording_id(recording_id);
        Ok(())
    } else {
        Err(PyTypeError::new_err(format!(
            "Invalid recording id - expected a UUID, got {recording_id:?}"
        )))
    }
}

#[pyfunction]
fn init(application_id: String, application_path: Option<PathBuf>) {
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

    global_session().set_application_id(ApplicationId(application_id), is_official_example);
}

#[pyfunction]
fn connect(addr: Option<String>) -> PyResult<()> {
    let addr = if let Some(addr) = addr {
        addr.parse()?
    } else {
        re_sdk_comms::default_server_addr()
    };
    global_session().connect(addr);
    Ok(())
}

/// Serve a web-viewer.
#[allow(clippy::unnecessary_wraps)] // False positive
#[pyfunction]
fn serve() -> PyResult<()> {
    #[cfg(feature = "web")]
    {
        global_session().serve();
        Ok(())
    }

    #[cfg(not(feature = "web"))]
    Err(PyRuntimeError::new_err(
        "The Rerun SDK was not compiled with the 'web' feature",
    ))
}

#[pyfunction]
fn shutdown(py: Python<'_>) {
    // Release the GIL in case any flushing behavior needs to
    // cleanup a python object.
    py.allow_threads(|| {
        re_log::debug!("Shutting down the Rerun SDK");
        let mut session = global_session();
        session.drop_msgs_if_disconnected();
        session.flush();
        session.disconnect();
    });
}

/// Disconnect from remote server (if any).
///
/// Subsequent log messages will be buffered and either sent on the next call to `connect`,
/// or shown with `show`.
#[cfg(feature = "re_viewer")]
#[pyfunction]
fn disconnect() {
    global_session().disconnect();
}

/// Show the buffered log data.
///
/// NOTE: currently this only works _once_.
/// Calling this function more than once is undefined behavior.
/// We will try to fix this in the future.
/// Blocked on <https://github.com/emilk/egui/issues/1918>.
#[cfg(feature = "re_viewer")]
#[pyfunction]
fn show() -> PyResult<()> {
    let mut session = global_session();
    if session.is_connected() {
        return Err(PyRuntimeError::new_err(
            "Can't show the log messages: Rerun was configured to send the data to a server!",
        ));
    }

    let log_messages = session.drain_log_messages_buffer();
    drop(session);

    if log_messages.is_empty() {
        re_log::info!("Nothing logged, so nothing to show");
        Ok(())
    } else {
        rerun_sdk::viewer::show(log_messages)
            .map_err(|err| PyRuntimeError::new_err(format!("Failed to show Rerun Viewer: {err}")))
    }
}

#[pyfunction]
fn save(path: &str) -> PyResult<()> {
    re_log::trace!("Saving file to {path:?}…");

    let mut session = global_session();
    if session.is_connected() {
        return Err(PyRuntimeError::new_err(
            "Can't show the log messages: Rerun was configured to send the data to a server!",
        ));
    }

    let log_messages = session.drain_log_messages_buffer();
    drop(session);

    if log_messages.is_empty() {
        re_log::info!("Nothing logged, so nothing to save");
    }

    if !path.ends_with(".rrd") {
        re_log::warn!("Expected path to end with .rrd, got {path:?}");
    }

    match std::fs::File::create(path) {
        Ok(file) => {
            if let Err(err) = re_log_types::encoding::encode(log_messages.iter(), file) {
                Err(PyRuntimeError::new_err(format!(
                    "Failed to write to file at {path:?}: {err}",
                )))
            } else {
                re_log::info!("Rerun data file saved to {path:?}");
                Ok(())
            }
        }
        Err(err) => Err(PyRuntimeError::new_err(format!(
            "Failed to create file at {path:?}: {err}",
        ))),
    }
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
    let mut session = global_session();
    let time_point = time(timeless);

    // We currently log arrow transforms from inside the bridge because we are
    // using glam and macaw to potentially do matrix-inversion as part of the
    // logging pipeline. Implementing these data-transforms consistently on the
    // python side will take a bit of additional work and testing to ensure we aren't
    // introducing new numerical issues.

    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path,
        time_point,
        vec![vec![transform].try_into().unwrap()],
    );

    let msg = bundle.try_into().unwrap();

    session.send(LogMsg::ArrowMsg(msg));

    Ok(())
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

    let mut session = global_session();
    let time_point = time(timeless);

    // We currently log view coordinates from inside the bridge because the code
    // that does matching and validation on different string representations is
    // non-trivial. Implementing this functionality on the python side will take
    // a bit of additional work and testing to ensure we aren't introducing new
    // conversion errors.
    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path,
        time_point,
        vec![vec![coordinates].try_into().unwrap()],
    );

    let msg = bundle.try_into().unwrap();

    session.send(LogMsg::ArrowMsg(msg));

    Ok(())
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
    index_buffers: Vec<Option<numpy::PyReadonlyArray1<'_, u32>>>,
    normal_buffers: Vec<Option<numpy::PyReadonlyArray1<'_, f32>>>,
    albedo_factors: Vec<Option<numpy::PyReadonlyArray1<'_, f32>>>,
    timeless: bool,
) -> PyResult<()> {
    let entity_path = parse_entity_path(entity_path_str)?;

    // Make sure we have as many position buffers as index buffers, etc.
    if position_buffers.len() != index_buffers.len()
        || position_buffers.len() != normal_buffers.len()
        || position_buffers.len() != albedo_factors.len()
    {
        return Err(PyTypeError::new_err(format!(
            "Top-level position/index/normal/albedo buffer arrays must be same the length, \
                got positions={}, indices={}, normals={}, albedo={} instead",
            position_buffers.len(),
            index_buffers.len(),
            normal_buffers.len(),
            albedo_factors.len(),
        )));
    }

    let mut session = global_session();

    let time_point = time(timeless);

    let mut meshes = Vec::with_capacity(position_buffers.len());
    for (i, positions) in position_buffers.into_iter().enumerate() {
        let albedo_factor = if let Some(v) = albedo_factors[i]
            .as_ref()
            .map(|albedo_factor| albedo_factor.as_array().to_vec())
        {
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

        let raw = RawMesh3D {
            mesh_id: MeshId::random(),
            positions: positions.as_array().to_vec(),
            indices: index_buffers[i]
                .as_ref()
                .map(|indices| indices.as_array().to_vec()),
            normals: normal_buffers[i]
                .as_ref()
                .map(|normals| normals.as_array().to_vec()),
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

    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path,
        time_point,
        vec![meshes.try_into().unwrap()],
    );

    let msg = bundle.try_into().unwrap();

    session.send(LogMsg::ArrowMsg(msg));

    Ok(())
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
    let bytes = bytes.into();
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

    let mut session = global_session();

    let time_point = time(timeless);

    let mesh3d = Mesh3D::Encoded(EncodedMesh3D {
        mesh_id: MeshId::random(),
        format,
        bytes,
        transform,
    });

    // We currently log `Mesh3D` from inside the bridge.
    //
    // Pyarrow handling of nested unions was causing more grief that it was
    // worth fighting with in the short term.
    //
    // TODO(jleibs) replace with python-native implementation

    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path,
        time_point,
        vec![vec![mesh3d].try_into().unwrap()],
    );

    let msg = bundle.try_into().unwrap();

    session.send(LogMsg::ArrowMsg(msg));

    Ok(())
}

/// Log an image file given its path on disk.
///
/// If no `img_format` is specified, we will try and guess it.
#[pyfunction]
#[pyo3(signature = (entity_path, img_path, img_format = None, timeless = false))]
fn log_image_file(
    entity_path: &str,
    img_path: PathBuf,
    img_format: Option<&str>,
    timeless: bool,
) -> PyResult<()> {
    let entity_path = parse_entity_path(entity_path)?;

    let img_bytes = std::fs::read(img_path)?;
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

    let mut session = global_session();

    let time_point = time(timeless);

    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path,
        time_point,
        vec![vec![re_log_types::component_types::Tensor {
            tensor_id: TensorId::random(),
            shape: vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
                TensorDimension::depth(3),
            ],
            data: re_log_types::component_types::TensorData::JPEG(img_bytes),
            meaning: re_log_types::component_types::TensorDataMeaning::Unknown,
            meter: None,
        }]
        .try_into()
        .unwrap()],
    );

    let msg = bundle.try_into().unwrap();

    session.send(LogMsg::ArrowMsg(msg));

    Ok(())
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
    let mut session = global_session();

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
    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path,
        time_point,
        vec![vec![annotation_context.clone()].try_into().unwrap()],
    );

    let msg = bundle.try_into().unwrap();

    session.send(LogMsg::ArrowMsg(msg));

    Ok(())
}

#[pyfunction]
fn log_cleared(entity_path: &str, recursive: bool) -> PyResult<()> {
    let entity_path = parse_entity_path(entity_path)?;
    let mut session = global_session();

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
    let msg = crate::arrow::build_chunk_from_components(&entity_path, components, &time(timeless))?;

    let mut session = global_session();
    session.send(msg);

    Ok(())
}
