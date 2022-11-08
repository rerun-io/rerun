#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use std::{borrow::Cow, io::Cursor, path::PathBuf};

use bytemuck::allocation::pod_collect_to_vec;
use itertools::Itertools as _;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::PyList,
};

use re_log_types::{LoggedData, *};

use crate::sdk::Sdk;

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
        time_point.0.insert(
            Timeline::new("log_time", TimeType::Time),
            Time::now().into(),
        );
        time_point
    }

    fn set_time(&mut self, timeline: Timeline, time_int: Option<TimeInt>) {
        if let Some(time_int) = time_int {
            self.time_point.0.insert(timeline, time_int);
        } else {
            self.time_point.0.remove(&timeline);
        }
    }
}

// ----------------------------------------------------------------------------

/// The python module is called "rerun_sdk".
#[pymodule]
fn rerun_sdk(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    Sdk::global().set_recording_id(default_recording_id(py));

    m.add_function(wrap_pyfunction!(main, m)?)?;

    m.add_function(wrap_pyfunction!(get_recording_id, m)?)?;
    m.add_function(wrap_pyfunction!(set_recording_id, m)?)?;

    m.add_function(wrap_pyfunction!(init, m)?)?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;

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
    m.add_function(wrap_pyfunction!(log_class_descriptions, m)?)?;

    m.add_function(wrap_pyfunction!(log_tensor_u8, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_u16, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_f32, m)?)?;

    m.add_function(wrap_pyfunction!(log_mesh_file, m)?)?;
    m.add_function(wrap_pyfunction!(log_image_file, m)?)?;
    m.add_function(wrap_pyfunction!(set_visible, m)?)?;

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
    use pyo3::types::{PyBytes, PyDict};
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

// ----------------------------------------------------------------------------

#[pyfunction]
fn main(argv: Vec<String>) -> PyResult<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(rerun::run(argv))
        .map_err(|err| PyRuntimeError::new_err(re_error::format(err)))
}

#[pyfunction]
fn get_recording_id() -> PyResult<String> {
    Sdk::global()
        .recording_id()
        .ok_or_else(|| PyTypeError::new_err("module has not been initialized"))
        .map(|recording_id| recording_id.to_string())
}

#[pyfunction]
fn set_recording_id(recording_id: &str) -> PyResult<()> {
    if let Ok(recording_id) = recording_id.parse() {
        Sdk::global().set_recording_id(recording_id);
        Ok(())
    } else {
        Err(PyTypeError::new_err(format!(
            "Invalid recording id - expected a UUID, got {:?}",
            recording_id
        )))
    }
}

#[pyfunction]
fn init(application_id: String) {
    Sdk::global().set_application_id(ApplicationId(application_id));
}

#[pyfunction]
fn connect(addr: Option<String>) -> PyResult<()> {
    let addr = if let Some(addr) = addr {
        addr.parse()?
    } else {
        re_sdk_comms::default_server_addr()
    };
    Sdk::global().connect(addr);
    Ok(())
}

/// Serve a web-viewer.
#[allow(clippy::unnecessary_wraps)] // False positive
#[pyfunction]
fn serve() -> PyResult<()> {
    #[cfg(feature = "web")]
    {
        Sdk::global().serve();
        Ok(())
    }

    #[cfg(not(feature = "web"))]
    Err(PyRuntimeError::new_err(
        "The Rerun SDK was not compiled with the 'web' feature",
    ))
}

/// Wait until all logged data have been sent to the remove server (if any).
#[pyfunction]
fn flush() {
    Sdk::global().flush();
}

/// Disconnect from remote server (if any).
///
/// Subsequent log messages will be buffered and either sent on the next call to `connect`,
/// or shown with `show`.
#[cfg(feature = "re_viewer")]
#[pyfunction]
fn disconnect() {
    Sdk::global().disconnect();
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
    let mut sdk = Sdk::global();
    if sdk.is_connected() {
        return Err(PyRuntimeError::new_err(
            "Can't show the log messages: Rerun was configured to send the data to a server!",
        ));
    }

    let log_messages = sdk.drain_log_messages_buffer();
    drop(sdk);

    if log_messages.is_empty() {
        re_log::info!("Nothing logged, so nothing to show");
    } else {
        let (tx, rx) = std::sync::mpsc::channel();
        for log_msg in log_messages {
            tx.send(log_msg).ok();
        }
        re_viewer::run_native_viewer_with_rx(rx);
    }

    Ok(())
}

#[pyfunction]
fn save(path: &str) -> PyResult<()> {
    re_log::trace!("Saving file to {path:?}…");

    let mut sdk = Sdk::global();
    if sdk.is_connected() {
        return Err(PyRuntimeError::new_err(
            "Can't show the log messages: Rerun was configured to send the data to a server!",
        ));
    }

    let log_messages = sdk.drain_log_messages_buffer();
    drop(sdk);

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
        image_from_cam: child_from_parent,
        resolution: Some(resolution),
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
    let mut sdk = Sdk::global();
    let time_point = time(timeless);
    sdk.send_data(
        &time_point,
        (&obj_path, "_transform"),
        LoggedData::Single(Data::Transform(transform)),
    );
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
    use re_log_types::coordinates::*;

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
    use re_log_types::coordinates::*;

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

    let mut sdk = Sdk::global();
    let obj_path = parse_obj_path(obj_path)?;
    let time_point = time(timeless);
    sdk.send_data(
        &time_point,
        (&obj_path, "_view_coordinates"),
        LoggedData::Single(Data::ViewCoordinates(coordinates)),
    );

    Ok(())
}

// ----------------------------------------------------------------------------

/// Log a text entry.
#[pyfunction]
fn log_text_entry(
    obj_path: &str,
    text: &str,
    level: Option<&str>,
    color: Option<Vec<u8>>,
    timeless: bool,
) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::TextEntry);

    let time_point = time(timeless);

    sdk.send_data(
        &time_point,
        (&obj_path, "body"),
        LoggedData::Single(Data::String(text.to_owned())),
    );

    if let Some(lvl) = level {
        sdk.send_data(
            &time_point,
            (&obj_path, "level"),
            LoggedData::Single(Data::String(lvl.to_owned())),
        );
    }

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------

/// Log a scalar.
#[pyfunction]
fn log_scalar(obj_path: &str, scalar: f64, color: Option<Vec<u8>>, timeless: bool) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Scalar);

    let time_point = time(timeless);

    sdk.send_data(
        &time_point,
        (&obj_path, "scalar"),
        LoggedData::Single(Data::F64(scalar)),
    );

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
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
    r: [f32; 4],
    color: Option<Vec<u8>>,
    label: Option<String>,
    timeless: bool,
) -> PyResult<()> {
    let rect_format = RectFormat::parse(rect_format)?;
    let bbox = rect_format.to_bbox(r);

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::BBox2D);

    let time_point = time(timeless);

    sdk.send_data(
        &time_point,
        (&obj_path, "bbox"),
        LoggedData::Single(Data::BBox2D(bbox)),
    );

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(label) = label {
        sdk.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    Ok(())
}

#[pyfunction]
fn log_rects(
    obj_path: &str,
    rect_format: &str,
    rects: numpy::PyReadonlyArrayDyn<'_, f32>,
    colors: numpy::PyReadonlyArrayDyn<'_, u8>,
    labels: Vec<String>,
    timeless: bool,
) -> PyResult<()> {
    // Note: we cannot early-out here on `rects.empty()`, beacause logging
    // an empty batch is same as deleting previous batch.

    let rect_format = RectFormat::parse(rect_format)?;

    let n = match rects.shape() {
        [0] => 0,
        [n, 4] => *n,
        shape => {
            return Err(PyTypeError::new_err(format!(
                "Expected Nx4 rects array; got {shape:?}"
            )));
        }
    };

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;

    sdk.register_type(obj_path.obj_type_path(), ObjectType::BBox2D);

    let indices = BatchIndex::SequentialIndex(n);

    let time_point = time(timeless);

    if !colors.is_empty() {
        let color_data = color_batch(&indices, colors)?;
        sdk.send_data(&time_point, (&obj_path, "color"), color_data);
    }

    match labels.len() {
        0 => {}
        1 => {
            sdk.send_data(
                &time_point,
                (&obj_path, "label"),
                LoggedData::BatchSplat(Data::String(labels[0].clone())),
            );
        }
        num_labels if num_labels == n => {
            sdk.send_data(
                &time_point,
                (&obj_path, "label"),
                LoggedData::Batch {
                    indices: indices.clone(),
                    data: DataVec::String(labels),
                },
            );
        }
        num_labels => {
            return Err(PyTypeError::new_err(format!(
                "Got {num_labels} labels for {n} rects"
            )));
        }
    }

    let rects = vec_from_np_array(&rects)
        .chunks_exact(4)
        .map(|r| rect_format.to_bbox([r[0], r[1], r[2], r[3]]))
        .collect();

    sdk.send_data(
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
#[pyfunction]
fn log_point(
    obj_path: &str,
    position: numpy::PyReadonlyArray1<'_, f32>,
    color: Option<Vec<u8>>,
    timeless: bool,
) -> PyResult<()> {
    let dim = match position.shape() {
        [2] => 2,
        [3] => 3,
        shape => {
            return Err(PyTypeError::new_err(format!(
                "Expected either a 2D or 3D position; got {shape:?}"
            )));
        }
    };

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;

    sdk.register_type(
        obj_path.obj_type_path(),
        if dim == 2 {
            ObjectType::Point2D
        } else {
            ObjectType::Point3D
        },
    );

    let time_point = time(timeless);

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    let position = vec_from_np_array(&position);
    let pos_data = match dim {
        2 => Data::Vec2([position[0], position[1]]),
        3 => Data::Vec3([position[0], position[1], position[2]]),
        _ => unreachable!(),
    };

    sdk.send_data(
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
#[pyfunction]
fn log_points(
    obj_path: &str,
    positions: numpy::PyReadonlyArray2<'_, f32>,
    colors: numpy::PyReadonlyArrayDyn<'_, u8>,
    timeless: bool,
) -> PyResult<()> {
    // Note: we cannot early-out here on `positions.empty()`, beacause logging
    // an empty batch is same as deleting previous batch.

    let (n, dim) = match positions.shape() {
        [n, 2] => (*n, 2),
        [n, 3] => (*n, 3),
        shape => {
            return Err(PyTypeError::new_err(format!(
                "Expected Nx2 or Nx3 positions array; got {shape:?}"
            )));
        }
    };

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;

    sdk.register_type(
        obj_path.obj_type_path(),
        if dim == 2 {
            ObjectType::Point2D
        } else {
            ObjectType::Point3D
        },
    );

    let indices = BatchIndex::SequentialIndex(n);

    let time_point = time(timeless);

    if !colors.is_empty() {
        let color_data = color_batch(&indices, colors)?;
        sdk.send_data(&time_point, (&obj_path, "color"), color_data);
    }

    let pos_data = match dim {
        2 => DataVec::Vec2(pod_collect_to_vec(&vec_from_np_array(&positions))),
        3 => DataVec::Vec3(pod_collect_to_vec(&vec_from_np_array(&positions))),
        _ => unreachable!(),
    };

    sdk.send_data(
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
    positions: numpy::PyReadonlyArray2<'_, f32>,
    stroke_width: Option<f32>,
    color: Option<Vec<u8>>,
    timeless: bool,
) -> PyResult<()> {
    if !matches!(positions.shape(), [_, 3]) {
        return Err(PyTypeError::new_err(format!(
            "Expected Nx3 positions array; got {:?}",
            positions.shape()
        )));
    }

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Path3D);

    let time_point = time(timeless);

    let positions = pod_collect_to_vec(&vec_from_np_array(&positions));

    sdk.send_data(
        &time_point,
        (&obj_path, "points"),
        LoggedData::Single(Data::DataVec(DataVec::Vec3(positions))),
    );

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(stroke_width) = stroke_width {
        sdk.send_data(
            &time_point,
            (&obj_path, "stroke_width"),
            LoggedData::Single(Data::F32(stroke_width)),
        );
    }

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

    let (dim, positions) = match positions.shape() {
        [_, 2] => (
            2,
            Data::DataVec(DataVec::Vec2(pod_collect_to_vec(&vec_from_np_array(
                &positions,
            )))),
        ),
        [_, 3] => (
            3,
            Data::DataVec(DataVec::Vec3(pod_collect_to_vec(&vec_from_np_array(
                &positions,
            )))),
        ),
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Expected Nx2 or Nx3 positions array; got {:?}",
                positions.shape()
            )));
        }
    };

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    let obj_type = if dim == 2 {
        ObjectType::LineSegments2D
    } else {
        ObjectType::LineSegments3D
    };
    sdk.register_type(obj_path.obj_type_path(), obj_type);

    let time_point = time(timeless);

    sdk.send_data(
        &time_point,
        (&obj_path, "points"),
        LoggedData::Single(positions),
    );

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(stroke_width) = stroke_width {
        sdk.send_data(
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
    origin: [f32; 3],
    vector: [f32; 3],
    color: Option<Vec<u8>>,
    label: Option<String>,
    width_scale: Option<f32>,
    timeless: bool,
) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Arrow3D);

    let time_point = time(timeless);

    let arrow = re_log_types::Arrow3D { origin, vector };

    sdk.send_data(
        &time_point,
        (&obj_path, "arrow3d"),
        LoggedData::Single(Data::Arrow3D(arrow)),
    );

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(width_scale) = width_scale {
        sdk.send_data(
            &time_point,
            (&obj_path, "width_scale"),
            LoggedData::Single(Data::F32(width_scale)),
        );
    }

    if let Some(label) = label {
        sdk.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    Ok(())
}

/// Log a 3D oriented bounding box, defined by its half size.
///
/// Optionally give it a label.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn log_obb(
    obj_path: &str,
    half_size: [f32; 3],
    position: [f32; 3],
    rotation_q: re_log_types::Quaternion,
    color: Option<Vec<u8>>,
    stroke_width: Option<f32>,
    label: Option<String>,
    timeless: bool,
) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Box3D);

    let time_point = time(timeless);

    let obb = re_log_types::Box3 {
        rotation: rotation_q,
        translation: position,
        half_size,
    };

    sdk.send_data(
        &time_point,
        (&obj_path, "obb"),
        LoggedData::Single(Data::Box3(obb)),
    );

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send_data(
            &time_point,
            (&obj_path, "color"),
            LoggedData::Single(Data::Color(color)),
        );
    }

    if let Some(stroke_width) = stroke_width {
        sdk.send_data(
            &time_point,
            (&obj_path, "stroke_width"),
            LoggedData::Single(Data::F32(stroke_width)),
        );
    }

    if let Some(label) = label {
        sdk.send_data(
            &time_point,
            (&obj_path, "label"),
            LoggedData::Single(Data::String(label)),
        );
    }

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_u8(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, u8>,
    names: Option<&PyList>,
    meter: Option<f32>,
    legend: Option<String>,
    timeless: bool,
) -> PyResult<()> {
    log_tensor(obj_path, img, names, meter, legend, timeless)
}

#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_u16(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, u16>,
    names: Option<&PyList>,
    meter: Option<f32>,
    legend: Option<String>,
    timeless: bool,
) -> PyResult<()> {
    log_tensor(obj_path, img, names, meter, legend, timeless)
}

#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_f32(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, f32>,
    names: Option<&PyList>,
    meter: Option<f32>,
    legend: Option<String>,
    timeless: bool,
) -> PyResult<()> {
    log_tensor(obj_path, img, names, meter, legend, timeless)
}

fn log_tensor<T: TensorDataTypeTrait + numpy::Element + bytemuck::Pod>(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, T>,
    names: Option<&PyList>,
    meter: Option<f32>,
    legend: Option<String>,
    timeless: bool,
) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Image);

    let time_point = time(timeless);

    let names: Option<Vec<String>> =
        names.map(|names| names.iter().map(|n| n.to_string()).collect());

    sdk.send_data(
        &time_point,
        (&obj_path, "tensor"),
        LoggedData::Single(Data::Tensor(
            re_tensor_ops::to_rerun_tensor(&img.as_array(), names)
                .map_err(|err| PyTypeError::new_err(err.to_string()))?,
        )),
    );

    if let Some(meter) = meter {
        sdk.send_data(
            &time_point,
            (&obj_path, "meter"),
            LoggedData::Single(Data::F32(meter)),
        );
    }

    if let Some(legend) = legend {
        sdk.send_data(
            &time_point,
            (&obj_path, "legend"),
            LoggedData::Single(Data::ObjPath(parse_obj_path(&legend)?)),
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------

#[pyfunction]
fn log_mesh_file(
    obj_path: &str,
    mesh_format: &str,
    bytes: &[u8],
    transform: numpy::PyReadonlyArray2<'_, f32>,
    timeless: bool,
) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path)?;
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

    let mut sdk = Sdk::global();

    sdk.register_type(obj_path.obj_type_path(), ObjectType::Mesh3D);

    let time_point = time(timeless);

    sdk.send_data(
        &time_point,
        (&obj_path, "mesh"),
        LoggedData::Single(Data::Mesh3D(Mesh3D::Encoded(EncodedMesh3D {
            format,
            bytes,
            transform,
        }))),
    );

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
    let ((w, h), data) = match img_format {
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

            (jpeg.dimensions(), TensorDataStore::Jpeg(img_bytes.into()))
        }
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Unsupported image format {img_format:?}. \
                Expected one of: JPEG"
            )))
        }
    };

    let mut sdk = Sdk::global();

    sdk.register_type(obj_path.obj_type_path(), ObjectType::Image);

    let time_point = time(timeless);

    sdk.send_data(
        &time_point,
        (&obj_path, "tensor"),
        LoggedData::Single(Data::Tensor(re_log_types::Tensor {
            shape: vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
                TensorDimension::depth(3),
            ],
            dtype: TensorDataType::U8,
            data,
        })),
    );

    Ok(())
}

/// Clear the visibility flag of an object
#[pyfunction]
fn set_visible(obj_path: &str, visibile: bool) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path)?;

    let mut sdk = Sdk::global();

    let time_point = time(false);

    sdk.send_data(
        &time_point,
        (&obj_path, "_visible"),
        LoggedData::Single(Data::Bool(visibile)),
    );

    Ok(())
}

// Unzip supports nested, but not 3 or 4-length parallel structures
// ((id, index), (label, color))
type UnzipSegMap = (
    (Vec<i32>, Vec<Index>),
    (Vec<Option<String>>, Vec<Option<[u8; 4]>>),
);

#[pyfunction]
fn log_class_descriptions(
    obj_path: &str,
    class_descriptions: Vec<(i32, Option<String>, Option<Vec<u8>>)>,
    timeless: bool,
) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;

    let ((ids, indices), (labels, colors)): UnzipSegMap = class_descriptions
        .iter()
        .map(|(id, label, color)| {
            let corrected_color = color
                .as_ref()
                .map(|color| convert_color(color.clone()).unwrap());
            (
                (id, Index::Integer(*id as i128)),
                (label.clone(), corrected_color),
            )
        })
        .unzip();

    // Avoid duplicate indices
    let dups: Vec<&i32> = ids.iter().duplicates().collect();
    if !dups.is_empty() {
        return Err(PyTypeError::new_err(format!(
            "ClassDescription contains duplicate ids {:?}",
            dups
        )));
    }

    sdk.register_type(obj_path.obj_type_path(), ObjectType::ClassDescription);

    let time_point = time(timeless);

    sdk.send_data(
        &time_point,
        (&obj_path, "id"),
        LoggedData::Batch {
            indices: BatchIndex::FullIndex(indices.clone()),
            data: DataVec::I32(ids),
        },
    );

    // Strip out any indices with unset labels
    let (label_indices, labels) = std::iter::zip(indices.clone(), labels)
        .filter_map(|(i, l)| Some((i, l?)))
        .unzip();

    sdk.send_data(
        &time_point,
        (&obj_path, "label"),
        LoggedData::Batch {
            indices: BatchIndex::FullIndex(label_indices),
            data: DataVec::String(labels),
        },
    );

    // Strip out any indices with unset colors
    let (color_indices, colors) = std::iter::zip(indices, colors)
        .filter_map(|(i, c)| Some((i, c?)))
        .unzip();

    sdk.send_data(
        &time_point,
        (&obj_path, "color"),
        LoggedData::Batch {
            indices: BatchIndex::FullIndex(color_indices),
            data: DataVec::Color(colors),
        },
    );

    Ok(())
}
