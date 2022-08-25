#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro

use bytemuck::allocation::pod_collect_to_vec;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use re_log_types::{LoggedData, *};

use crate::sdk::Sdk;

/// The python module is called "rerun_sdk".
#[pymodule]
fn rerun_sdk(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;

    #[cfg(feature = "re_viewer")]
    {
        m.add_function(wrap_pyfunction!(disconnect, m)?)?;
        m.add_function(wrap_pyfunction!(show, m)?)?;
    }

    m.add_function(wrap_pyfunction!(set_time_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(set_time_nanos, m)?)?;

    m.add_function(wrap_pyfunction!(set_space_up, m)?)?;

    m.add_function(wrap_pyfunction!(log_rect, m)?)?;
    m.add_function(wrap_pyfunction!(log_points, m)?)?;
    m.add_function(wrap_pyfunction!(log_path, m)?)?;
    m.add_function(wrap_pyfunction!(log_line_segments, m)?)?;

    m.add_function(wrap_pyfunction!(log_tensor_u8, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_u16, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_f32, m)?)?;

    m.add_function(wrap_pyfunction!(log_mesh_file, m)?)?;

    Sdk::global().begin_new_recording();

    Ok(())
}

fn parse_obj_path_comps(obj_path: &str) -> PyResult<Vec<ObjPathComp>> {
    re_log_types::parse_obj_path(obj_path).map_err(|err| PyTypeError::new_err(err.to_string()))
}

fn parse_obj_path(obj_path: &str) -> PyResult<ObjPath> {
    parse_obj_path_comps(obj_path).map(|comps| ObjPath::from(ObjPathBuilder::new(comps)))
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
fn show() {
    let mut sdk = Sdk::global();
    if sdk.is_connected() {
        tracing::error!(
            "Can't show the log messages of Rerun: it was configured to send the data to a server!"
        );
    } else {
        let log_messages = sdk.drain_log_messages_buffer();
        drop(sdk);

        if log_messages.is_empty() {
            tracing::info!("Nothing logged, so nothing to show");
        } else {
            let (tx, rx) = std::sync::mpsc::channel();
            for log_msg in log_messages {
                tx.send(log_msg).ok();
            }
            re_viewer::run_native_viewer_with_rx(rx);
        }
    }
}

/// Set the current time globally. Used for all subsequent logging,
/// until the next call to `set_time_sequence`.
///
/// For instance: `set_time_sequence("frame_nr", frame_nr)`.
///
/// You can remove a time source again using `set_time_sequence("frame_nr", None)`.
#[pyfunction]
fn set_time_sequence(time_source: &str, sequence: Option<i64>) {
    Sdk::global().set_time(
        TimeSource::new(time_source, TimeType::Sequence),
        sequence.map(TimeValue::sequence),
    );
}

#[pyfunction]
fn set_time_seconds(time_source: &str, seconds: Option<f64>) {
    Sdk::global().set_time(
        TimeSource::new(time_source, TimeType::Time),
        seconds.map(|secs| Time::from_seconds_since_epoch(secs).into()),
    );
}

#[pyfunction]
fn set_time_nanos(time_source: &str, ns: Option<i64>) {
    Sdk::global().set_time(
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

    let time_point = sdk.now();

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point,
        data_path: DataPath::new(space_obj_path, "up".into()),
        data: LoggedData::Single(Data::Vec3(up)),
    }));

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

/// Log a 2D bounding box.
///
/// Optionally give it a label.
/// If no `space` is given, the space name "2D" will be used.
#[pyfunction]
fn log_rect(
    obj_path: &str,
    left_top: [f32; 2],
    width_height: [f32; 2],
    color: Option<Vec<u8>>,
    label: Option<String>,
    space: Option<String>,
) -> PyResult<()> {
    let [x, y] = left_top;
    let [w, h] = width_height;
    let min = [x, y];
    let max = [x + w, y + h];

    let mut sdk = Sdk::global();

    let obj_path = parse_obj_path(obj_path)?;
    sdk.register_type(obj_path.obj_type_path(), ObjectType::BBox2D);

    let time_point = sdk.now();

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "bbox".into()),
        data: LoggedData::Single(Data::BBox2D(BBox2D { min, max })),
    }));

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "color".into()),
            data: LoggedData::Single(Data::Color(color)),
        }));
    }

    if let Some(label) = label {
        sdk.send(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "label".into()),
            data: LoggedData::Single(Data::String(label)),
        }));
    }

    let space = space.unwrap_or_else(|| "2D".to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point,
        data_path: DataPath::new(obj_path, "space".into()),
        data: LoggedData::Single(Data::Space(space.into())),
    }));

    Ok(())
}

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
    if positions.is_empty() {
        return Ok(());
    }

    let (num_pos, dim) = match positions.shape() {
        [n, 2] => (*n, 2),
        [n, 3] => (*n, 3),
        shape => {
            return Err(PyTypeError::new_err(format!(
                "Expected Nx2 or Nx3 positions array; got {shape:?}"
            )));
        }
    };

    let mut sdk = Sdk::global();

    let root_path = ObjPathBuilder::new(parse_obj_path_comps(obj_path)?);
    let point_path = ObjPath::from(&root_path / ObjPathComp::Index(Index::Placeholder));

    let mut type_path = root_path.obj_type_path();
    type_path.push(TypePathComp::Index);

    sdk.register_type(
        &type_path,
        if dim == 2 {
            ObjectType::Point2D
        } else {
            ObjectType::Point3D
        },
    );

    let indices: Vec<_> = (0..num_pos).map(|i| Index::Sequence(i as _)).collect();

    let time_point = sdk.now();

    if !colors.is_empty() {
        let mut send_color = |data| {
            sdk.send(LogMsg::DataMsg(DataMsg {
                msg_id: MsgId::random(),
                time_point: time_point.clone(),
                data_path: DataPath::new(point_path.clone(), "color".into()),
                data,
            }));
        };

        match colors.shape() {
            [3] | [1, 3] => {
                // A single RGB
                let slice = colors.as_slice().unwrap(); // TODO(emilk): Handle non-contiguous arrays
                assert_eq!(slice.len(), 3);
                let color = [slice[0], slice[1], slice[2], 255];
                send_color(LoggedData::BatchSplat(Data::Color(color)));
            }
            [4] | [1, 4] => {
                // A single RGBA
                let slice = colors.as_slice().unwrap(); // TODO(emilk): Handle non-contiguous arrays
                assert_eq!(slice.len(), 4);
                let color = [slice[0], slice[1], slice[2], slice[3]];
                send_color(LoggedData::BatchSplat(Data::Color(color)));
            }
            [_, 3] => {
                // RGB, RGB, RGB, …
                let colors: Vec<[u8; 4]> = colors
                    .as_slice()
                    .unwrap()
                    .chunks_exact(3)
                    .into_iter()
                    .map(|chunk| [chunk[0], chunk[1], chunk[2], 255])
                    .collect();

                send_color(LoggedData::Batch {
                    indices: indices.clone(),
                    data: DataVec::Color(colors),
                });
            }
            [_, 4] => {
                // RGBA, RGBA, RGBA, …
                send_color(LoggedData::Batch {
                    indices: indices.clone(),
                    data: DataVec::Color(pod_collect_to_vec(colors.as_slice().unwrap())),
                });
            }
            shape => {
                return Err(PyTypeError::new_err(format!(
                    "Expected Nx4 color array; got {shape:?}"
                )));
            }
        };
    }

    // TODO(emilk): handle non-contigious arrays
    let pos_data = match dim {
        2 => DataVec::Vec2(pod_collect_to_vec(positions.as_slice().unwrap())),
        3 => DataVec::Vec3(pod_collect_to_vec(positions.as_slice().unwrap())),
        _ => unreachable!(),
    };

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(point_path.clone(), "pos".into()),
        data: LoggedData::Batch {
            indices,
            data: pos_data,
        },
    }));

    let space = space.unwrap_or_else(|| if dim == 2 { "2D" } else { "3D" }.to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point,
        data_path: DataPath::new(point_path, "space".into()),
        data: LoggedData::BatchSplat(Data::Space(space.into())),
    }));

    Ok(())
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

    let time_point = sdk.now();

    let positions = pod_collect_to_vec(positions.as_slice().unwrap());

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "points".into()),
        data: LoggedData::Single(Data::DataVec(DataVec::Vec3(positions))),
    }));

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "color".into()),
            data: LoggedData::Single(Data::Color(color)),
        }));
    }

    if let Some(stroke_width) = stroke_width {
        sdk.send(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "stroke_width".into()),
            data: LoggedData::Single(Data::F32(stroke_width)),
        }));
    }

    let space = space.unwrap_or_else(|| "3D".to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point,
        data_path: DataPath::new(obj_path, "space".into()),
        data: LoggedData::Single(Data::Space(space.into())),
    }));

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
            Data::DataVec(DataVec::Vec2(pod_collect_to_vec(
                positions.as_slice().unwrap(),
            ))),
        ),
        [_, 3] => (
            3,
            Data::DataVec(DataVec::Vec3(pod_collect_to_vec(
                positions.as_slice().unwrap(),
            ))),
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

    let time_point = sdk.now();

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "points".into()),
        data: LoggedData::Single(positions),
    }));

    if let Some(color) = color {
        let color = convert_color(color)?;
        sdk.send(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "color".into()),
            data: LoggedData::Single(Data::Color(color)),
        }));
    }

    if let Some(stroke_width) = stroke_width {
        sdk.send(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "stroke_width".into()),
            data: LoggedData::Single(Data::F32(stroke_width)),
        }));
    }

    let space = space.unwrap_or_else(|| if dim == 2 { "2D" } else { "3D" }.to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point,
        data_path: DataPath::new(obj_path, "space".into()),
        data: LoggedData::Single(Data::Space(space.into())),
    }));

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

    let time_point = sdk.now();

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "tensor".into()),
        data: LoggedData::Single(Data::Tensor(to_rerun_tensor(&img))),
    }));

    let space = space.unwrap_or_else(|| "2D".to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "space".into()),
        data: LoggedData::Single(Data::Space(space.into())),
    }));

    if let Some(meter) = meter {
        sdk.send(LogMsg::DataMsg(DataMsg {
            msg_id: MsgId::random(),
            time_point,
            data_path: DataPath::new(obj_path, "meter".into()),
            data: LoggedData::Single(Data::F32(meter)),
        }));
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
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    } else {
        if transform.shape() != [4, 4] {
            return Err(PyTypeError::new_err(format!(
                "Expected a 4x4 transformation matrix, got shape={:?}",
                transform.shape()
            )));
        }

        let get = |row, col| *transform.get([row, col]).unwrap();

        [
            [get(0, 0), get(1, 0), get(2, 0), get(3, 0)], // col 0
            [get(0, 1), get(1, 1), get(2, 1), get(3, 1)], // col 1
            [get(0, 2), get(1, 2), get(2, 2), get(3, 2)], // col 2
            [get(0, 3), get(1, 3), get(2, 3), get(3, 3)], // col 3
        ]
    };
    let space = space.unwrap_or_else(|| "3D".to_owned());

    let mut sdk = Sdk::global();

    sdk.register_type(obj_path.obj_type_path(), ObjectType::Mesh3D);

    let time_point = sdk.now();

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "mesh".into()),
        data: LoggedData::Single(Data::Mesh3D(Mesh3D::Encoded(EncodedMesh3D {
            format,
            bytes,
            transform,
        }))),
    }));

    sdk.send(LogMsg::DataMsg(DataMsg {
        msg_id: MsgId::random(),
        time_point,
        data_path: DataPath::new(obj_path, "space".into()),
        data: LoggedData::Single(Data::Space(space.into())),
    }));

    Ok(())
}
