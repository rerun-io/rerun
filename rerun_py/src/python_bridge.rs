#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use std::{borrow::Cow, io::Cursor, path::PathBuf};

use bytemuck::allocation::pod_collect_to_vec;
use itertools::Itertools as _;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyDict, PyList},
};

use re_log_types::{
    context, coordinates,
    field_types::{ClassId, KeypointId, Label, TensorDimension, TensorId},
    msg_bundle::MsgBundle,
    AnnotationContext, ApplicationId, BBox2D, BatchIndex, Data, DataVec, EncodedMesh3D, Index,
    LogMsg, LoggedData, Mesh3D, MeshFormat, MeshId, MsgId, ObjPath, ObjectType, PathOp,
    RecordingId, TensorDataStore, TensorDataType, Time, TimeInt, TimePoint, TimeType, Timeline,
    ViewCoordinates,
};

use rerun_sdk::global_session;

use crate::arrow::get_registered_fields;

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
        time_point.insert(
            Timeline::new("log_time", TimeType::Time),
            Time::now().into(),
        );
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
    re_log::setup_native_logging();

    global_session().set_recording_id(default_recording_id(py));

    m.add_function(wrap_pyfunction!(main, m)?)?;

    m.add_function(wrap_pyfunction!(get_registered_fields, m)?)?;

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

    m.add_function(wrap_pyfunction!(log_text_entry, m)?)?;
    m.add_function(wrap_pyfunction!(log_scalar, m)?)?;

    m.add_function(wrap_pyfunction!(log_rect, m)?)?;
    m.add_function(wrap_pyfunction!(log_rects, m)?)?;

    m.add_function(wrap_pyfunction!(log_arrow, m)?)?;
    m.add_function(wrap_pyfunction!(log_unknown_transform, m)?)?;
    m.add_function(wrap_pyfunction!(log_rigid3, m)?)?;
    m.add_function(wrap_pyfunction!(log_pinhole, m)?)?;

    m.add_function(wrap_pyfunction!(log_view_coordinates_xyz, m)?)?;
    m.add_function(wrap_pyfunction!(log_view_coordinates_up_handedness, m)?)?;

    m.add_function(wrap_pyfunction!(log_point, m)?)?;
    m.add_function(wrap_pyfunction!(log_points, m)?)?;
    m.add_function(wrap_pyfunction!(log_path, m)?)?;
    m.add_function(wrap_pyfunction!(log_line_segments, m)?)?;
    m.add_function(wrap_pyfunction!(log_obb, m)?)?;
    m.add_function(wrap_pyfunction!(log_annotation_context, m)?)?;

    m.add_function(wrap_pyfunction!(log_tensor, m)?)?;

    m.add_function(wrap_pyfunction!(log_mesh_file, m)?)?;
    m.add_function(wrap_pyfunction!(log_image_file, m)?)?;
    m.add_function(wrap_pyfunction!(log_cleared, m)?)?;
    m.add_function(wrap_pyfunction!(log_arrow_msg, m)?)?;
    m.add_function(wrap_pyfunction!(arrow_log_gate, m)?)?;
    m.add_function(wrap_pyfunction!(classic_log_gate, m)?)?;

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

fn parse_obj_path(obj_path: &str) -> PyResult<ObjPath> {
    let components = re_log_types::parse_obj_path(obj_path)
        .map_err(|err| PyTypeError::new_err(err.to_string()))?;
    if components.is_empty() {
        Err(PyTypeError::new_err(
            "You cannot log to the root {obj_path:?}",
        ))
    } else {
        Ok(ObjPath::from(components))
    }
}

fn vec_from_np_array<'a, T: numpy::Element, D: numpy::ndarray::Dimension>(
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

fn time(timeless: bool) -> TimePoint {
    if timeless {
        TimePoint::timeless()
    } else {
        ThreadInfo::thread_now()
    }
}

// TODO(emilk): ideally we would pass `Index` as something type-safe from Python.
fn parse_identifiers(identifiers: Vec<String>) -> PyResult<Vec<Index>> {
    let identifiers = identifiers
        .into_iter()
        .map(parse_index)
        .collect::<Result<Vec<_>, _>>()?;
    let as_set: ahash::HashSet<_> = identifiers.iter().collect();
    if as_set.len() == identifiers.len() {
        Ok(identifiers)
    } else {
        Err(PyTypeError::new_err(
            "The identifiers contained duplicates, but they need to be unique!",
        ))
    }
}

fn parse_index(s: String) -> PyResult<Index> {
    use std::str::FromStr as _;

    if let Some(seq) = s.strip_prefix('#') {
        if let Ok(sequence) = u64::from_str(seq) {
            Ok(Index::Sequence(sequence))
        } else {
            Err(PyTypeError::new_err(format!(
                "Expected index starting with '#' to be a sequence, got {s:?}"
            )))
        }
    } else if let Ok(integer) = i128::from_str(&s) {
        Ok(Index::Integer(integer))
    } else if let Ok(uuid) = uuid::Uuid::parse_str(&s) {
        Ok(Index::Uuid(uuid))
    } else {
        Ok(Index::String(s))
    }
}

// ----------------------------------------------------------------------------
#[pyfunction]
fn main(argv: Vec<String>) -> PyResult<u8> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(rerun::run(argv))
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
            "Invalid recording id - expected a UUID, got {:?}",
            recording_id
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
/// For instance: `set_time_sequence("frame_nr", frame_nr)`.
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
            "Expected color to be of length 3 or 4, got {:?}",
            color
        ))),
    }
}

#[pyfunction]
fn log_unknown_transform(obj_path: &str, timeless: bool) -> PyResult<()> {
    let transform = re_log_types::Transform::Unknown;
    log_transform(obj_path, transform, timeless)
}

#[pyfunction]
fn log_rigid3(
    obj_path: &str,
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

    log_transform(obj_path, transform, timeless)
}

#[pyfunction]
fn log_pinhole(
    obj_path: &str,
    resolution: [f32; 2],
    child_from_parent: [[f32; 3]; 3],
    timeless: bool,
) -> PyResult<()> {
    let transform = re_log_types::Transform::Pinhole(re_log_types::Pinhole {
        image_from_cam: child_from_parent.into(),
        resolution: Some(resolution.into()),
    });

    log_transform(obj_path, transform, timeless)
}

fn log_transform(
    obj_path: &str,
    transform: re_log_types::Transform,
    timeless: bool,
) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path)?;
    if obj_path.len() == 1 {
        // Stop people from logging a transform to a root-object, such as "world" (which doesn't have a parent).
        return Err(PyTypeError::new_err("Transforms are between a child object and its parent, so root objects cannot have transforms"));
    }
    let mut session = global_session();
    let time_point = time(timeless);

    // We currently log arrow transforms from inside the bridge because we are
    // using glam and macaw to potentially do matrix-inversion as part of the
    // logging pipeline. Implementing these data-transforms consistently on the
    // python side will take a bit of additional work and testing to ensure we aren't
    // introducing new numerical issues.
    if session.arrow_log_gate() {
        let arrow_path = session.arrow_prefix_obj_path(obj_path.clone());
        let bundle = MsgBundle::new(
            MsgId::random(),
            arrow_path,
            time_point.clone(),
            vec![vec![transform.clone()].try_into().unwrap()],
        );

        let msg = bundle.try_into().unwrap();

        session.send(LogMsg::ArrowMsg(msg));
    }

    if session.classic_log_gate() {
        let obj_path = session.classic_prefix_obj_path(obj_path);
        session.send_data(
            &time_point,
            (&obj_path, "_transform"),
            LoggedData::Single(Data::Transform(transform)),
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------

#[pyfunction]
fn log_view_coordinates_xyz(
    obj_path: &str,
    xyz: &str,
    right_handed: Option<bool>,
    timeless: bool,
) -> PyResult<()> {
    use re_log_types::coordinates::{Handedness, ViewCoordinates};

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

    log_view_coordinates(obj_path, coordinates, timeless)
}

#[pyfunction]
fn log_view_coordinates_up_handedness(
    obj_path: &str,
    up: &str,
    right_handed: bool,
    timeless: bool,
) -> PyResult<()> {
    use re_log_types::coordinates::{Handedness, SignedAxis3, ViewCoordinates};

    let up = up.parse::<SignedAxis3>().map_err(PyTypeError::new_err)?;
    let handedness = Handedness::from_right_handed(right_handed);
    let coordinates = ViewCoordinates::from_up_and_handedness(up, handedness);

    log_view_coordinates(obj_path, coordinates, timeless)
}

fn log_view_coordinates(
    obj_path: &str,
    coordinates: ViewCoordinates,
    timeless: bool,
) -> PyResult<()> {
    if coordinates.handedness() == Some(coordinates::Handedness::Left) {
        re_log::warn_once!("Left-handed coordinate systems are not yet fully supported by Rerun");
    }

    let mut session = global_session();
    let obj_path = parse_obj_path(obj_path)?;
    let time_point = time(timeless);

    // We currently log view coordinates from inside the bridge because the code
    // that does matching and validation on different string representations is
    // non-trivial. Implementing this functionality on the python side will take
    // a bit of additional work and testing to ensure we aren't introducing new
    // conversion errors.
    if session.arrow_log_gate() {
        let arrow_path = session.arrow_prefix_obj_path(obj_path.clone());
        let bundle = MsgBundle::new(
            MsgId::random(),
            arrow_path,
            time_point.clone(),
            vec![vec![coordinates].try_into().unwrap()],
        );

        let msg = bundle.try_into().unwrap();

        session.send(LogMsg::ArrowMsg(msg));
    }

    if session.classic_log_gate() {
        let obj_path = session.classic_prefix_obj_path(obj_path);
        session.send_data(
            &time_point,
            (&obj_path, "_view_coordinates"),
            LoggedData::Single(Data::ViewCoordinates(coordinates)),
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------

/// Log a text entry.
#[pyfunction]
fn log_text_entry(
    obj_path: &str,
    text: Option<&str>,
    level: Option<&str>,
    color: Option<Vec<u8>>,
    timeless: bool,
) -> PyResult<()> {
    let mut session = global_session();

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);
    session.register_type(obj_path.obj_type_path(), ObjectType::TextEntry);

    let time_point = time(timeless);

    let Some(text) = text
    else {
        session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
        return Ok(());
    };

    if let Some(lvl) = level {
        session.send_data(
            &time_point,
            (&obj_path, "level"),
            LoggedData::Single(Data::String(lvl.to_owned())),
        );
    }

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    session.send_data(
        &time_point,
        (&obj_path, "body"),
        LoggedData::Single(Data::String(text.to_owned())),
    );

    Ok(())
}

// ----------------------------------------------------------------------------

/// Log a scalar.
//
// TODO(cmc): Note that this will unnecessarily duplicate data in the very likely case that
// the caller logs a bunch of points all with the same attribute(s).
// E.g. if the caller logs 1000 points with `color=[255,0,0]`, then we will in fact store that
// information a 1000 times for no good reason.
//
// In the future, I'm hopeful that Arrow will allow us to automagically identify and eliminate
// that kind of deduplication at the lowest layer.
// If not, there is still the option of deduplicating either on the SDK-side (but then
// multiprocess can become problematic) or immediately upon entry on the server-side.
#[pyfunction]
fn log_scalar(
    obj_path: &str,
    scalar: f64,
    label: Option<String>,
    color: Option<Vec<u8>>,
    radius: Option<f32>,
    scattered: Option<bool>,
) -> PyResult<()> {
    let mut session = global_session();

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);
    session.register_type(obj_path.obj_type_path(), ObjectType::Scalar);

    let time_point = time(false);

    session.send_data(
        &time_point,
        (&obj_path, "scalar"),
        LoggedData::Single(Data::F64(scalar)),
    );

    if let Some(label) = label {
        session.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(radius) = radius {
        session.send_data(
            &time_point,
            (&obj_path, "radius"),
            LoggedData::Single(Data::F32(radius)),
        );
    }

    if let Some(scattered) = scattered {
        session.send_data(
            &time_point,
            (&obj_path, "scattered"),
            LoggedData::Single(Data::Bool(scattered)),
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------

#[allow(non_camel_case_types, clippy::upper_case_acronyms)] // we follow Python style
enum RectFormat {
    XYWH,
    YXHW,
    XYXY,
    YXYX,
    XCYCWH,
    XCYCW2H2,
}

impl RectFormat {
    fn parse(rect_format: &str) -> PyResult<RectFormat> {
        match rect_format {
            "XYWH" => Ok(Self::XYWH),
            "YXHW" => Ok(Self::YXHW),
            "XYXY" => Ok(Self::XYXY),
            "YXYX" => Ok(Self::YXYX),
            "XCYCWH" => Ok(Self::XCYCWH),
            "XCYCW2H2" => Ok(Self::XCYCW2H2),
            _ => Err(PyTypeError::new_err(format!(
                "Unknown RectFormat: {rect_format:?}. \
                Expected one of: XYWH YXHW XYXY XCYCWH XCYCW2H2"
            ))),
        }
    }

    fn to_bbox(&self, r: [f32; 4]) -> BBox2D {
        let (min, max) = match (self, r) {
            (Self::XYWH, [x, y, w, h]) | (Self::YXHW, [y, x, h, w]) => ([x, y], [x + w, y + h]),
            (Self::XYXY, [x0, y0, x1, y1]) | (Self::YXYX, [y0, x0, y1, x1]) => ([x0, y0], [x1, y1]),
            (Self::XCYCWH, [xc, yc, w, h]) => {
                ([xc - w / 2.0, yc - h / 2.0], [xc + w / 2.0, yc + h / 2.0])
            }
            (Self::XCYCW2H2, [xc, yc, half_w, half_h]) => {
                ([xc - half_w, yc - half_h], [xc + half_w, yc + half_h])
            }
        };
        BBox2D { min, max }
    }
}

/// Log a 2D bounding box.
///
/// Optionally give it a label.
#[pyfunction]
fn log_rect(
    obj_path: &str,
    rect_format: &str,
    rect: Option<[f32; 4]>,
    color: Option<Vec<u8>>,
    label: Option<String>,
    class_id: Option<i32>,
    timeless: bool,
) -> PyResult<()> {
    let rect_format = RectFormat::parse(rect_format)?;

    let mut session = global_session();

    let time_point = time(timeless);

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);

    let Some(rect) = rect
    else {
        session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
        return Ok(());
    };

    let bbox = rect_format.to_bbox(rect);

    session.register_type(obj_path.obj_type_path(), ObjectType::BBox2D);

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(label) = label {
        session.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    if let Some(class_id) = class_id {
        session.send_data(
            &time_point,
            (&obj_path, "class_id"),
            LoggedData::Single(Data::I32(class_id)),
        );
    }

    session.send_data(
        &time_point,
        (&obj_path, "bbox"),
        LoggedData::Single(Data::BBox2D(bbox)),
    );

    Ok(())
}

fn log_labels(
    session: &mut rerun_sdk::Session,
    obj_path: &ObjPath,
    labels: Vec<String>,
    indices: &BatchIndex,
    time_point: &TimePoint,
    num_objects: usize,
) -> PyResult<()> {
    match labels.len() {
        0 => Ok(()),
        1 => {
            session.send_data(
                time_point,
                (obj_path, "label"),
                LoggedData::BatchSplat(Data::String(labels[0].clone())),
            );
            Ok(())
        }
        num_labels if num_labels == num_objects => {
            session.send_data(
                time_point,
                (obj_path, "label"),
                LoggedData::Batch {
                    indices: indices.clone(),
                    data: DataVec::String(labels),
                },
            );
            Ok(())
        }
        num_labels => Err(PyTypeError::new_err(format!(
            "Got {num_labels} labels for {num_objects} objects"
        ))),
    }
}

#[derive(Copy, Clone)]
enum IdType {
    ClassId,
    KeypointId,
}

impl IdType {
    fn field_name(self) -> &'static str {
        match self {
            IdType::ClassId => "class_id",
            IdType::KeypointId => "keypoint_id",
        }
    }
}

fn log_ids(
    session: &mut rerun_sdk::Session,
    obj_path: &ObjPath,
    ids: &numpy::PyReadonlyArrayDyn<'_, u16>,
    id_type: IdType,
    indices: &BatchIndex,
    time_point: &TimePoint,
    num_objects: usize,
) -> PyResult<()> {
    match ids.len() {
        0 => Ok(()),
        1 => {
            session.send_data(
                time_point,
                (obj_path, id_type.field_name()),
                LoggedData::BatchSplat(Data::I32(ids.to_vec().unwrap()[0] as i32)),
            );
            Ok(())
        }
        num_ids if num_ids == num_objects => {
            session.send_data(
                time_point,
                (obj_path, id_type.field_name()),
                LoggedData::Batch {
                    indices: indices.clone(),
                    // TODO(andreas): We don't have a u16 data type, do late conversion here. This will likely go away with new data model.
                    data: DataVec::I32(ids.cast(false).unwrap().to_vec().unwrap()),
                },
            );
            Ok(())
        }
        num_ids => Err(PyTypeError::new_err(format!(
            "Got {num_ids} class_id for {num_objects} objects"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn log_rects(
    obj_path: &str,
    rect_format: &str,
    rects: numpy::PyReadonlyArrayDyn<'_, f32>,
    identifiers: Vec<String>,
    colors: numpy::PyReadonlyArrayDyn<'_, u8>,
    labels: Vec<String>,
    class_ids: numpy::PyReadonlyArrayDyn<'_, u16>,
    timeless: bool,
) -> PyResult<()> {
    // Note: we cannot early-out here on `rects.empty()`, beacause logging
    // an empty batch is same as deleting previous batch.
    let mut session = global_session();

    let time_point = time(timeless);

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);

    if rects.is_empty() {
        session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
        return Ok(());
    }

    let rect_format = RectFormat::parse(rect_format)?;

    let n = match rects.shape() {
        [n, 4] => *n,
        shape => {
            return Err(PyTypeError::new_err(format!(
                "Expected Nx4 rects array; got {shape:?}"
            )));
        }
    };

    session.register_type(obj_path.obj_type_path(), ObjectType::BBox2D);

    let indices = if identifiers.is_empty() {
        BatchIndex::SequentialIndex(n)
    } else {
        let indices = parse_identifiers(identifiers)?;
        if indices.len() != n {
            return Err(PyTypeError::new_err(format!(
                "Get {n} rectangles but {} identifiers",
                indices.len()
            )));
        }
        BatchIndex::FullIndex(indices)
    };

    if !colors.is_empty() {
        let color_data = color_batch(&indices, colors)?;
        session.send_data(&time_point, (&obj_path, "color"), color_data);
    }

    log_labels(&mut session, &obj_path, labels, &indices, &time_point, n)?;
    log_ids(
        &mut session,
        &obj_path,
        &class_ids,
        IdType::ClassId,
        &indices,
        &time_point,
        n,
    )?;

    let rects = vec_from_np_array(&rects)
        .chunks_exact(4)
        .map(|r| rect_format.to_bbox([r[0], r[1], r[2], r[3]]))
        .collect();

    session.send_data(
        &time_point,
        (&obj_path, "bbox"),
        LoggedData::Batch {
            indices,
            data: DataVec::BBox2D(rects),
        },
    );

    Ok(())
}

// ----------------------------------------------------------------------------

/// Log a 2D or 3D point.
///
/// `position` is either 2x1 or 3x1.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn log_point(
    obj_path: &str,
    position: Option<numpy::PyReadonlyArray1<'_, f32>>,
    radius: Option<f32>,
    color: Option<Vec<u8>>,
    label: Option<String>,
    class_id: Option<u16>,
    keypoint_id: Option<u16>,
    timeless: bool,
) -> PyResult<()> {
    let mut session = global_session();

    let time_point = time(timeless);

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);

    let Some(position) = position
    else {
        session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
        return Ok(());
    };

    let obj_type = match position.shape() {
        [2] => ObjectType::Point2D,
        [3] => ObjectType::Point3D,
        shape => {
            return Err(PyTypeError::new_err(format!(
                "Expected either a 2D or 3D position; got {shape:?}"
            )));
        }
    };

    session.register_type(obj_path.obj_type_path(), obj_type);

    if let Some(radius) = radius {
        session.send_data(
            &time_point,
            (&obj_path, "radius"),
            LoggedData::Single(Data::F32(radius)),
        );
    }

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(label) = label {
        session.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    if let Some(class_id) = class_id {
        session.send_data(
            &time_point,
            (&obj_path, "class_id"),
            LoggedData::Single(Data::I32(class_id as _)),
        );
    }

    if let Some(keypoint_id) = keypoint_id {
        session.send_data(
            &time_point,
            (&obj_path, "keypoint_id"),
            LoggedData::Single(Data::I32(keypoint_id as _)),
        );
    }

    let position = vec_from_np_array(&position);
    let pos_data = match obj_type {
        ObjectType::Point2D => Data::Vec2([position[0], position[1]]),
        ObjectType::Point3D => Data::Vec3([position[0], position[1], position[2]]),
        _ => unreachable!(),
    };

    session.send_data(
        &time_point,
        (&obj_path, "pos"),
        LoggedData::Single(pos_data),
    );

    Ok(())
}

/// positions: Nx2 or Nx3 array
/// * `colors.len() == 0`: no colors
/// * `colors.len() == 1`: same color for all points
/// * `colors.len() == positions.len()`: a color per point
#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn log_points(
    obj_path: &str,
    positions: numpy::PyReadonlyArrayDyn<'_, f32>,
    identifiers: Vec<String>,
    colors: numpy::PyReadonlyArrayDyn<'_, u8>,
    radii: numpy::PyReadonlyArrayDyn<'_, f32>,
    labels: Vec<String>,
    class_ids: numpy::PyReadonlyArrayDyn<'_, u16>,
    keypoint_ids: numpy::PyReadonlyArrayDyn<'_, u16>,
    timeless: bool,
) -> PyResult<()> {
    // Note: we cannot early-out here on `positions.empty()`, beacause logging
    // an empty batch is same as deleting previous batch.

    let mut session = global_session();

    let time_point = time(timeless);

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);

    if positions.is_empty() {
        session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
        return Ok(());
    }

    let (n, obj_type) = match positions.shape() {
        [n, 2] => (*n, ObjectType::Point2D),
        [n, 3] => (*n, ObjectType::Point3D),
        shape => {
            return Err(PyTypeError::new_err(format!(
                "Expected Nx2 or Nx3 positions array; got {shape:?}"
            )));
        }
    };

    session.register_type(obj_path.obj_type_path(), obj_type);

    let indices = if identifiers.is_empty() {
        BatchIndex::SequentialIndex(n)
    } else {
        let indices = parse_identifiers(identifiers)?;
        if indices.len() != n {
            return Err(PyTypeError::new_err(format!(
                "Get {n} positions but {} identifiers",
                indices.len()
            )));
        }
        BatchIndex::FullIndex(indices)
    };

    let time_point = time(timeless);

    if !colors.is_empty() {
        let color_data = color_batch(&indices, colors)?;
        session.send_data(&time_point, (&obj_path, "color"), color_data);
    }

    match radii.len() {
        0 => {}
        1 => {
            session.send_data(
                &time_point,
                (&obj_path, "radius"),
                LoggedData::BatchSplat(Data::F32(radii.to_vec().unwrap()[0])),
            );
        }
        num_ids if num_ids == n => {
            session.send_data(
                &time_point,
                (&obj_path, "radius"),
                LoggedData::Batch {
                    indices: indices.clone(),
                    data: DataVec::F32(radii.cast(false).unwrap().to_vec().unwrap()),
                },
            );
        }
        num_ids => {
            return Err(PyTypeError::new_err(format!(
                "Got {num_ids} radii for {n} objects"
            )));
        }
    }

    log_labels(&mut session, &obj_path, labels, &indices, &time_point, n)?;
    log_ids(
        &mut session,
        &obj_path,
        &class_ids,
        IdType::ClassId,
        &indices,
        &time_point,
        n,
    )?;
    log_ids(
        &mut session,
        &obj_path,
        &keypoint_ids,
        IdType::KeypointId,
        &indices,
        &time_point,
        n,
    )?;

    let pos_data = match obj_type {
        ObjectType::Point2D => DataVec::Vec2(pod_collect_to_vec(&vec_from_np_array(&positions))),
        ObjectType::Point3D => DataVec::Vec3(pod_collect_to_vec(&vec_from_np_array(&positions))),
        _ => unreachable!(),
    };

    session.send_data(
        &time_point,
        (&obj_path, "pos"),
        LoggedData::Batch {
            indices,
            data: pos_data,
        },
    );

    Ok(())
}

fn color_batch(
    indices: &BatchIndex,
    colors: numpy::PyReadonlyArrayDyn<'_, u8>,
) -> PyResult<LoggedData> {
    match colors.shape() {
        [3] | [1, 3] => {
            // A single RGB
            let colors = vec_from_np_array(&colors);
            assert_eq!(colors.len(), 3);
            let color = [colors[0], colors[1], colors[2], 255];
            Ok(LoggedData::BatchSplat(Data::Color(color)))
        }
        [4] | [1, 4] => {
            // A single RGBA
            let colors = vec_from_np_array(&colors);
            assert_eq!(colors.len(), 4);
            let color = [colors[0], colors[1], colors[2], colors[3]];
            Ok(LoggedData::BatchSplat(Data::Color(color)))
        }
        [_, 3] => {
            // RGB, RGB, RGB, …
            let colors = vec_from_np_array(&colors)
                .chunks_exact(3)
                .into_iter()
                .map(|chunk| [chunk[0], chunk[1], chunk[2], 255])
                .collect_vec();

            if colors.len() == indices.len() {
                Ok(LoggedData::Batch {
                    indices: indices.clone(),
                    data: DataVec::Color(colors),
                })
            } else {
                Err(PyTypeError::new_err(format!(
                    "Expected {} colors, got {}",
                    indices.len(),
                    colors.len()
                )))
            }
        }
        [_, 4] => {
            // RGBA, RGBA, RGBA, …

            let colors = pod_collect_to_vec(&vec_from_np_array(&colors));

            if colors.len() == indices.len() {
                Ok(LoggedData::Batch {
                    indices: indices.clone(),
                    data: DataVec::Color(colors),
                })
            } else {
                Err(PyTypeError::new_err(format!(
                    "Expected {} colors, got {}",
                    indices.len(),
                    colors.len()
                )))
            }
        }
        shape => Err(PyTypeError::new_err(format!(
            "Expected Nx4 color array; got {shape:?}"
        ))),
    }
}

#[pyfunction]
fn log_path(
    obj_path: &str,
    positions: Option<numpy::PyReadonlyArray2<'_, f32>>,
    stroke_width: Option<f32>,
    color: Option<Vec<u8>>,
    timeless: bool,
) -> PyResult<()> {
    let mut session = global_session();

    let time_point = time(timeless);

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);

    let Some(positions) = positions
    else {
        session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
        return Ok(());
    };

    if !matches!(positions.shape(), [_, 3]) {
        return Err(PyTypeError::new_err(format!(
            "Expected Nx3 positions array; got {:?}",
            positions.shape()
        )));
    }

    session.register_type(obj_path.obj_type_path(), ObjectType::Path3D);

    let positions = pod_collect_to_vec(&vec_from_np_array(&positions));

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(stroke_width) = stroke_width {
        session.send_data(
            &time_point,
            (&obj_path, "stroke_width"),
            LoggedData::Single(Data::F32(stroke_width)),
        );
    }

    session.send_data(
        &time_point,
        (&obj_path, "points"),
        LoggedData::Single(Data::DataVec(DataVec::Vec3(positions))),
    );

    Ok(())
}

#[pyfunction]
fn log_line_segments(
    obj_path: &str,
    positions: numpy::PyReadonlyArray2<'_, f32>,
    stroke_width: Option<f32>,
    color: Option<Vec<u8>>,
    timeless: bool,
) -> PyResult<()> {
    let num_points = positions.shape()[0];
    if num_points % 2 != 0 {
        return Err(PyTypeError::new_err(format!(
            "Expected an even number of points; got {num_points} points"
        )));
    }

    let mut session = global_session();

    let time_point = time(timeless);

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);

    if positions.is_empty() {
        session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
        return Ok(());
    }

    let obj_type = match positions.shape() {
        [_, 2] => ObjectType::LineSegments2D,
        [_, 3] => ObjectType::LineSegments3D,
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Expected Nx2 or Nx3 positions array; got {:?}",
                positions.shape()
            )));
        }
    };

    session.register_type(obj_path.obj_type_path(), obj_type);

    let positions = match obj_type {
        ObjectType::LineSegments2D => Data::DataVec(DataVec::Vec2(pod_collect_to_vec(
            &vec_from_np_array(&positions),
        ))),
        ObjectType::LineSegments3D => Data::DataVec(DataVec::Vec3(pod_collect_to_vec(
            &vec_from_np_array(&positions),
        ))),
        _ => unreachable!(),
    };

    session.send_data(
        &time_point,
        (&obj_path, "points"),
        LoggedData::Single(positions),
    );

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(stroke_width) = stroke_width {
        session.send_data(
            &time_point,
            (&obj_path, "stroke_width"),
            LoggedData::Single(Data::F32(stroke_width)),
        );
    }

    Ok(())
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn log_arrow(
    obj_path: &str,
    origin: Option<[f32; 3]>,
    vector: Option<[f32; 3]>,
    color: Option<Vec<u8>>,
    label: Option<String>,
    width_scale: Option<f32>,
    timeless: bool,
) -> PyResult<()> {
    let mut session = global_session();

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);

    session.register_type(obj_path.obj_type_path(), ObjectType::Arrow3D);

    let time_point = time(timeless);

    let arrow = match (origin, vector) {
        (Some(origin), Some(vector)) => re_log_types::Arrow3D {
            origin: origin.into(),
            vector: vector.into(),
        },
        (None, None) => {
            // None, None means to clear the arrow
            session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
            return Ok(());
        }
        _ => {
            return Err(PyTypeError::new_err(
                "log_arrow requires both origin and vector if either is not None",
            ))
        }
    };

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(width_scale) = width_scale {
        session.send_data(
            &time_point,
            (&obj_path, "width_scale"),
            LoggedData::Single(Data::F32(width_scale)),
        );
    }

    if let Some(label) = label {
        session.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    session.send_data(
        &time_point,
        (&obj_path, "arrow3d"),
        LoggedData::Single(Data::Arrow3D(arrow)),
    );

    Ok(())
}

/// Log a 3D oriented bounding box, defined by its half size.
///
/// Optionally give it a label.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn log_obb(
    obj_path: &str,
    half_size: Option<[f32; 3]>,
    position: Option<[f32; 3]>,
    rotation_q: Option<re_log_types::Quaternion>,
    color: Option<Vec<u8>>,
    stroke_width: Option<f32>,
    label: Option<String>,
    class_id: Option<u16>,
    timeless: bool,
) -> PyResult<()> {
    let mut session = global_session();

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);
    session.register_type(obj_path.obj_type_path(), ObjectType::Box3D);

    let time_point = time(timeless);

    let obb =
        match (rotation_q, position, half_size) {
            (Some(rotation), Some(translation), Some(half_size)) => re_log_types::Box3 {
                rotation,
                translation,
                half_size,
            },
            (None, None, None) => {
                // None, None means to clear the arrow
                session.send_path_op(&time_point, PathOp::ClearFields(obj_path));
                return Ok(());
            }
            _ => return Err(PyTypeError::new_err(
                "log_obb requires all of half_size, position, and rotation to be provided or None",
            )),
        };

    if let Some(color) = color {
        let color = convert_color(color)?;
        session.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(stroke_width) = stroke_width {
        session.send_data(
            &time_point,
            (&obj_path, "stroke_width"),
            LoggedData::Single(Data::F32(stroke_width)),
        );
    }

    if let Some(label) = label {
        session.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    if let Some(class_id) = class_id {
        session.send_data(
            &time_point,
            (&obj_path, "class_id"),
            LoggedData::Single(Data::I32(class_id as _)),
        );
    }

    session.send_data(
        &time_point,
        (&obj_path, "obb"),
        LoggedData::Single(Data::Box3(obb)),
    );

    Ok(())
}

// TODO(jleibs): This shadows [`re_log_types::TensorDataMeaning`]
//
#[pyclass]
#[derive(Clone, Debug)]
enum TensorDataMeaning {
    Unknown,
    ClassId,
}

fn tensor_extract_helper(
    any: &PyAny,
    names: Option<Vec<String>>,
    meaning: re_log_types::field_types::TensorDataMeaning,
) -> Result<re_log_types::ClassicTensor, re_tensor_ops::TensorCastError> {
    if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, u8>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, u16>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, u32>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, u64>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, i8>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, i16>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, i32>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, i64>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, half::f16>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, f32>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else if let Ok(tensor) = any.extract::<numpy::PyReadonlyArrayDyn<'_, f64>>() {
        re_tensor_ops::to_rerun_tensor(&tensor.as_array(), names, meaning)
    } else {
        Err(re_tensor_ops::TensorCastError::UnsupportedDataType)
    }
}

#[pyfunction]
fn log_tensor(
    obj_path: &str,
    img: &PyAny,
    names: Option<&PyList>,
    meter: Option<f32>,
    meaning: Option<TensorDataMeaning>,
    timeless: bool,
) -> PyResult<()> {
    let mut session = global_session();

    let obj_path = parse_obj_path(obj_path)?;
    let obj_path = session.classic_prefix_obj_path(obj_path);
    session.register_type(obj_path.obj_type_path(), ObjectType::Image);

    let time_point = time(timeless);

    let names: Option<Vec<String>> =
        names.map(|names| names.iter().map(|n| n.to_string()).collect());

    // Convert from pyclass TensorDataMeaning -> re_log_types
    let meaning = match meaning {
        Some(TensorDataMeaning::ClassId) => re_log_types::field_types::TensorDataMeaning::ClassId,
        _ => re_log_types::field_types::TensorDataMeaning::Unknown,
    };

    let tensor = tensor_extract_helper(img, names, meaning)
        .map_err(|err| PyTypeError::new_err(err.to_string()))?;

    session.send_data(
        &time_point,
        (&obj_path, "tensor"),
        LoggedData::Single(Data::Tensor(tensor)),
    );

    if let Some(meter) = meter {
        session.send_data(
            &time_point,
            (&obj_path, "meter"),
            LoggedData::Single(Data::F32(meter)),
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------

#[pyfunction]
fn log_mesh_file(
    obj_path_str: &str,
    mesh_format: &str,
    bytes: &[u8],
    transform: numpy::PyReadonlyArray2<'_, f32>,
    timeless: bool,
) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path_str)?;
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
    if session.arrow_log_gate() {
        let arrow_path = session.arrow_prefix_obj_path(obj_path.clone());

        let bundle = MsgBundle::new(
            MsgId::random(),
            arrow_path,
            time_point.clone(),
            vec![vec![mesh3d.clone()].try_into().unwrap()],
        );

        let msg = bundle.try_into().unwrap();

        session.send(LogMsg::ArrowMsg(msg));
    }

    if session.classic_log_gate() {
        let obj_path = session.classic_prefix_obj_path(obj_path);
        session.register_type(obj_path.obj_type_path(), ObjectType::Mesh3D);
        session.send_data(
            &time_point,
            (&obj_path, "mesh"),
            LoggedData::Single(Data::Mesh3D(mesh3d)),
        );
    }

    Ok(())
}

/// Log an image file given its path on disk.
///
/// If no `img_format` is specified, we will try and guess it.
#[pyfunction]
fn log_image_file(
    obj_path: &str,
    img_path: PathBuf,
    img_format: Option<&str>,
    timeless: bool,
) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path)?;

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

    if session.arrow_log_gate() {
        let arrow_path = session.arrow_prefix_obj_path(obj_path.clone());
        let bundle = MsgBundle::new(
            MsgId::random(),
            arrow_path,
            time_point.clone(),
            vec![vec![re_log_types::field_types::Tensor {
                tensor_id: TensorId::random(),
                shape: vec![
                    TensorDimension::height(h as _),
                    TensorDimension::width(w as _),
                    TensorDimension::depth(3),
                ],
                data: re_log_types::field_types::TensorData::JPEG(img_bytes.clone()),
                meaning: re_log_types::field_types::TensorDataMeaning::Unknown,
                meter: None,
            }]
            .try_into()
            .unwrap()],
        );

        let msg = bundle.try_into().unwrap();

        session.send(LogMsg::ArrowMsg(msg));
    }

    if session.classic_log_gate() {
        let obj_path = session.classic_prefix_obj_path(obj_path);
        session.register_type(obj_path.obj_type_path(), ObjectType::Image);
        session.send_data(
            &time_point,
            (&obj_path, "tensor"),
            LoggedData::Single(Data::Tensor(re_log_types::ClassicTensor::new(
                TensorId::random(),
                vec![
                    TensorDimension::height(h as _),
                    TensorDimension::width(w as _),
                    TensorDimension::depth(3),
                ],
                TensorDataType::U8,
                re_log_types::field_types::TensorDataMeaning::Unknown,
                TensorDataStore::Jpeg(img_bytes.into()),
            ))),
        );
    }

    Ok(())
}

#[derive(FromPyObject)]
struct AnnotationInfoTuple(u16, Option<String>, Option<Vec<u8>>);

impl From<AnnotationInfoTuple> for context::AnnotationInfo {
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
    obj_path_str: &str,
    class_descriptions: Vec<ClassDescriptionTuple>,
    timeless: bool,
) -> PyResult<()> {
    let mut session = global_session();

    // We normally disallow logging to root, but we make an exception for class_descriptions
    let obj_path = if obj_path_str == "/" {
        ObjPath::root()
    } else {
        parse_obj_path(obj_path_str)?
    };

    let mut annotation_context = AnnotationContext::default();

    for (info, keypoint_annotations, keypoint_skeleton_edges) in class_descriptions {
        annotation_context
            .class_map
            .entry(ClassId(info.0))
            .or_insert_with(|| context::ClassDescription {
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
    if session.arrow_log_gate() {
        let arrow_path = session.arrow_prefix_obj_path(obj_path.clone());
        let bundle = MsgBundle::new(
            MsgId::random(),
            arrow_path,
            time_point.clone(),
            vec![vec![annotation_context.clone()].try_into().unwrap()],
        );

        let msg = bundle.try_into().unwrap();

        session.send(LogMsg::ArrowMsg(msg));
    }

    if session.classic_log_gate() {
        let obj_path = session.classic_prefix_obj_path(obj_path);
        session.send_data(
            &time_point,
            (&obj_path, "_annotation_context"),
            LoggedData::Single(Data::AnnotationContext(annotation_context)),
        );
    }

    Ok(())
}

#[pyfunction]
fn log_cleared(obj_path: &str, recursive: bool) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path)?;
    let mut session = global_session();

    let time_point = time(false);

    if session.arrow_log_gate() {
        let obj_path = session.arrow_prefix_obj_path(obj_path.clone());
        session.send_path_op(&time_point, PathOp::clear(recursive, obj_path));
    }

    if session.classic_log_gate() {
        let obj_path = session.classic_prefix_obj_path(obj_path);
        session.send_path_op(&time_point, PathOp::clear(recursive, obj_path));
    }

    Ok(())
}

#[pyfunction]
fn log_arrow_msg(obj_path: &str, components: &PyDict, timeless: bool) -> PyResult<()> {
    let obj_path = {
        let session = global_session();
        let obj_path = parse_obj_path(obj_path)?;
        session.arrow_prefix_obj_path(obj_path)
    };

    // It's important that we don't hold the session lock while building our arrow component.
    // the API we call to back through pyarrow temporarily releases the GIL, which can cause
    // cause a deadlock.
    let msg = crate::arrow::build_chunk_from_components(&obj_path, components, &time(timeless))?;

    let mut session = global_session();
    session.send(msg);

    Ok(())
}

#[pyfunction]
fn classic_log_gate() -> bool {
    let session = global_session();
    session.classic_log_gate()
}

#[pyfunction]
fn arrow_log_gate() -> bool {
    let session = global_session();
    session.arrow_log_gate()
}
