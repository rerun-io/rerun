#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pufunction] macro

use bytemuck::allocation::pod_collect_to_vec;
use itertools::Itertools as _;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

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

    pub fn set_thread_time(time_source: TimeSource, time_int: Option<TimeInt>) {
        Self::with(|ti| ti.set_time(time_source, time_int));
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
            TimeSource::new("log_time", TimeType::Time),
            Time::now().into(),
        );
        time_point
    }

    fn set_time(&mut self, time_source: TimeSource, time_int: Option<TimeInt>) {
        if let Some(time_int) = time_int {
            self.time_point.0.insert(time_source, time_int);
        } else {
            self.time_point.0.remove(&time_source);
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

    m.add_function(wrap_pyfunction!(get_recording_id, m)?)?;
    m.add_function(wrap_pyfunction!(set_recording_id, m)?)?;

    m.add_function(wrap_pyfunction!(connect, m)?)?;
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

    m.add_function(wrap_pyfunction!(set_space_up, m)?)?;

    m.add_function(wrap_pyfunction!(log_rect, m)?)?;
    m.add_function(wrap_pyfunction!(log_rects, m)?)?;

    m.add_function(wrap_pyfunction!(log_camera, m)?)?;
    m.add_function(wrap_pyfunction!(log_points, m)?)?;
    m.add_function(wrap_pyfunction!(log_path, m)?)?;
    m.add_function(wrap_pyfunction!(log_line_segments, m)?)?;

    m.add_function(wrap_pyfunction!(log_tensor_u8, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_u16, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_f32, m)?)?;

    m.add_function(wrap_pyfunction!(log_mesh_file, m)?)?;

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
    // TODO(emilk): are there any security conserns with leaking authkey?
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

fn parse_obj_path_comps(obj_path: &str) -> PyResult<Vec<ObjPathComp>> {
    re_log_types::parse_obj_path(obj_path).map_err(|err| PyTypeError::new_err(err.to_string()))
}

fn parse_obj_path(obj_path: &str) -> PyResult<ObjPath> {
    parse_obj_path_comps(obj_path).map(ObjPath::from)
}

fn vec_from_np_array<T: numpy::Element, D: numpy::ndarray::Dimension>(
    array: &numpy::PyReadonlyArray<'_, T, D>,
) -> Vec<T> {
    let array = array.as_array();
    if let Some(slice) = array.to_slice() {
        slice.to_vec()
    } else {
        array.iter().cloned().collect()
    }
}

// ----------------------------------------------------------------------------

#[pyfunction]
fn get_recording_id() -> String {
    let recording_id = Sdk::global()
        .recording_id()
        .expect("module has not been initialized");
    recording_id.to_string()
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
fn connect(addr: Option<String>) -> PyResult<()> {
    let addr = if let Some(addr) = addr {
        addr.parse()?
    } else {
        re_sdk_comms::default_server_addr()
    };
    Sdk::global().connect(addr);
    Ok(())
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
fn show() -> Result<(), PyErr> {
    let mut sdk = Sdk::global();
    if sdk.is_connected() {
        return Err(PyTypeError::new_err(
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
fn save(path: &str) -> Result<(), PyErr> {
    re_log::trace!("Saving file to {path:?}…");

    let mut sdk = Sdk::global();
    if sdk.is_connected() {
        return Err(PyTypeError::new_err(
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
                Err(PyTypeError::new_err(format!(
                    "Failed to write to file at {path:?}: {err}",
                )))
            } else {
                re_log::info!("Rerun data file saved to {path:?}");
                Ok(())
            }
        }
        Err(err) => Err(PyTypeError::new_err(format!(
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
/// You can remove a time source again using `set_time_sequence("frame_nr", None)`.
#[pyfunction]
fn set_time_sequence(time_source: &str, sequence: Option<i64>) {
    ThreadInfo::set_thread_time(
        TimeSource::new(time_source, TimeType::Sequence),
        sequence.map(TimeInt::from),
    );
}

#[pyfunction]
fn set_time_seconds(time_source: &str, seconds: Option<f64>) {
    ThreadInfo::set_thread_time(
        TimeSource::new(time_source, TimeType::Time),
        seconds.map(|secs| Time::from_seconds_since_epoch(secs).into()),
    );
}

#[pyfunction]
fn set_time_nanos(time_source: &str, ns: Option<i64>) {
    ThreadInfo::set_thread_time(
        TimeSource::new(time_source, TimeType::Time),
        ns.map(|ns| Time::from_ns_since_epoch(ns).into()),
    );
}

/// Set the preferred up-axis for a given 3D space.
#[pyfunction]
fn set_space_up(space_obj_path: &str, up: [f32; 3]) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let space_obj_path = parse_obj_path(space_obj_path)?;
    sdk.register_type(space_obj_path.obj_type_path(), ObjectType::Space);

    sdk.send_data(
        &TimePoint::timeless(),
        (&space_obj_path, "up"),
        LoggedData::Single(Data::Vec3(up)),
    );

    Ok(())
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

/// Log a 3D camera
#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn log_camera(
    obj_path: &str,
    resolution: [f32; 2],
    intrinsics: [[f32; 3]; 3],
    rotation_q: re_log_types::Quaternion,
    position: [f32; 3],
    camera_space_convention: &str,
    space: Option<String>,
    target_space: Option<String>,
) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path)?;

    let convention = match camera_space_convention {
        "XRightYUpZBack" => re_log_types::CameraSpaceConvention::XRightYUpZBack,
        "XRightYDownZFwd" => re_log_types::CameraSpaceConvention::XRightYDownZFwd,
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Unknown camera space convetions format {camera_space_convention:?}.
                Expected one of: XRightYUpZBack, XRightYDownZFwd"
            )));
        }
    };

    let target_space = if let Some(target_space) = target_space {
        Some(parse_obj_path(&target_space)?)
    } else {
        None
    };

    let camera = re_log_types::Camera {
        rotation: rotation_q,
        position,
        intrinsics: Some(intrinsics),
        resolution: Some(resolution),
        camera_space_convention: convention,
        target_space,
    };

    let mut sdk = Sdk::global();

    sdk.register_type(obj_path.obj_type_path(), ObjectType::Camera);

    let time_point = ThreadInfo::thread_now();

    sdk.send_data(
        &time_point,
        (&obj_path, "camera"),
        LoggedData::Single(Data::Camera(camera)),
    );

    let space = space.unwrap_or_else(|| "3D".to_owned());
    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::Single(Data::Space(parse_obj_path(&space)?)),
    );

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
/// If no `space` is given, the space name "2D" will be used.
#[pyfunction]
fn log_rect(
    obj_path: &str,
    rect_format: &str,
    r: [f32; 4],
    color: Option<Vec<u8>>,
    label: Option<String>,
    space: Option<String>,
) -> PyResult<()> {
    let rect_format = RectFormat::parse(rect_format)?;
    let bbox = rect_format.to_bbox(r);

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::BBox2D);

    let time_point = ThreadInfo::thread_now();

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

    let space = space.unwrap_or_else(|| "2D".to_owned());
    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::Single(Data::Space(parse_obj_path(&space)?)),
    );

    Ok(())
}

#[pyfunction]
fn log_rects(
    obj_path: &str,
    rect_format: &str,
    rects: numpy::PyReadonlyArray2<'_, f32>,
    colors: numpy::PyReadonlyArrayDyn<'_, u8>,
    labels: Vec<String>,
    space: Option<String>,
) -> PyResult<()> {
    // Note: we cannot early-out here on `rects.empty()`, beacause logging
    // an empty batch is same as deleting previous batch.

    let rect_format = RectFormat::parse(rect_format)?;

    let n = match rects.shape() {
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

    let indices: Vec<_> = (0..n).map(|i| Index::Sequence(i as _)).collect();

    let time_point = ThreadInfo::thread_now();

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

    let space = space.unwrap_or_else(|| "2D".to_owned());
    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::BatchSplat(Data::Space(parse_obj_path(&space)?)),
    );

    Ok(())
}

// ----------------------------------------------------------------------------

/// positions: Nx2 or Nx3 array
/// * `colors.len() == 0`: no colors
/// * `colors.len() == 1`: same color for all points
/// * `colors.len() == positions.len()`: a color per point
///
/// If no `space` is given, the space name "2D" or "3D" will be used,
/// depending on the dimensionality of the data.
#[pyfunction]
fn log_points(
    obj_path: &str,
    positions: numpy::PyReadonlyArray2<'_, f32>,
    colors: numpy::PyReadonlyArrayDyn<'_, u8>,
    space: Option<String>,
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

    let indices: Vec<_> = (0..n).map(|i| Index::Sequence(i as _)).collect();

    let time_point = ThreadInfo::thread_now();

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

    let space = space.unwrap_or_else(|| if dim == 2 { "2D" } else { "3D" }.to_owned());
    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::BatchSplat(Data::Space(parse_obj_path(&space)?)),
    );

    Ok(())
}

fn color_batch(
    indices: &Vec<Index>,
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
    space: Option<String>,
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

    let time_point = ThreadInfo::thread_now();

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

    let space = space.unwrap_or_else(|| "3D".to_owned());
    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::Single(Data::Space(parse_obj_path(&space)?)),
    );

    Ok(())
}

#[pyfunction]
fn log_line_segments(
    obj_path: &str,
    positions: numpy::PyReadonlyArray2<'_, f32>,
    stroke_width: Option<f32>,
    color: Option<Vec<u8>>,
    space: Option<String>,
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

    let time_point = ThreadInfo::thread_now();

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

    let space = space.unwrap_or_else(|| if dim == 2 { "2D" } else { "3D" }.to_owned());
    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::Single(Data::Space(parse_obj_path(&space)?)),
    );

    Ok(())
}

/// If no `space` is given, the space name "2D" will be used.
#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_u8(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, u8>,
    meter: Option<f32>,
    space: Option<String>,
) -> PyResult<()> {
    log_tensor(obj_path, img, meter, space)
}

/// If no `space` is given, the space name "2D" will be used.
#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_u16(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, u16>,
    meter: Option<f32>,
    space: Option<String>,
) -> PyResult<()> {
    log_tensor(obj_path, img, meter, space)
}

/// If no `space` is given, the space name "2D" will be used.
#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_f32(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, f32>,
    meter: Option<f32>,
    space: Option<String>,
) -> PyResult<()> {
    log_tensor(obj_path, img, meter, space)
}

/// If no `space` is given, the space name "2D" will be used.
fn log_tensor<T: TensorDataTypeTrait + numpy::Element + bytemuck::Pod>(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, T>,
    meter: Option<f32>,
    space: Option<String>,
) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Image);

    let time_point = ThreadInfo::thread_now();

    sdk.send_data(
        &time_point,
        (&obj_path, "tensor"),
        LoggedData::Single(Data::Tensor(to_rerun_tensor(&img))),
    );

    let space = space.unwrap_or_else(|| "2D".to_owned());
    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::Single(Data::Space(parse_obj_path(&space)?)),
    );

    if let Some(meter) = meter {
        sdk.send_data(
            &time_point,
            (&obj_path, "meter"),
            LoggedData::Single(Data::F32(meter)),
        );
    }

    Ok(())
}

fn to_rerun_tensor<T: TensorDataTypeTrait + numpy::Element + bytemuck::Pod>(
    img: &numpy::PyReadonlyArrayDyn<'_, T>,
) -> Tensor {
    let vec = img.to_owned_array().into_raw_vec();
    let vec = bytemuck::allocation::try_cast_vec(vec)
        .unwrap_or_else(|(_err, vec)| bytemuck::allocation::pod_collect_to_vec(&vec));

    Tensor {
        shape: img.shape().iter().map(|&d| d as u64).collect(),
        dtype: T::DTYPE,
        data: TensorDataStore::Dense(vec),
    }
}

#[pyfunction]
fn log_mesh_file(
    obj_path: &str,
    mesh_format: &str,
    bytes: &[u8],
    transform: numpy::PyReadonlyArray2<'_, f32>,
    space: Option<String>,
) -> PyResult<()> {
    let obj_path = parse_obj_path(obj_path)?;
    let format = match mesh_format {
        "GLB" => MeshFormat::Glb,
        "GLTF" => MeshFormat::Gltf,
        "OBJ" => MeshFormat::Obj,
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Unknown mesh format {mesh_format:?}. Expected one of: GLB, GLTF, OBJ"
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
    let space = space.unwrap_or_else(|| "3D".to_owned());

    let mut sdk = Sdk::global();

    sdk.register_type(obj_path.obj_type_path(), ObjectType::Mesh3D);

    let time_point = ThreadInfo::thread_now();

    sdk.send_data(
        &time_point,
        (&obj_path, "mesh"),
        LoggedData::Single(Data::Mesh3D(Mesh3D::Encoded(EncodedMesh3D {
            format,
            bytes,
            transform,
        }))),
    );

    sdk.send_data(
        &time_point,
        (&obj_path, "space"),
        LoggedData::Single(Data::Space(parse_obj_path(&space)?)),
    );

    Ok(())
}
