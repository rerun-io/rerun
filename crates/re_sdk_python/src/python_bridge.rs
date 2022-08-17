#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value
#![allow(clippy::borrow_deref_ref)] // False positive due to #[pufunction] macro

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use re_log_types::{LoggedData, *};

use crate::sdk::Sdk;

/// The python module is called "rerun_sdk".
#[pymodule]
fn rerun_sdk(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    m.add_function(wrap_pyfunction!(info, m)?)?;
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

    m.add_function(wrap_pyfunction!(log_bbox, m)?)?;
    m.add_function(wrap_pyfunction!(log_points_rs, m)?)?;

    m.add_function(wrap_pyfunction!(log_tensor_u8, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_u16, m)?)?;
    m.add_function(wrap_pyfunction!(log_tensor_f32, m)?)?;
    Ok(())
}

#[pyfunction]
fn info() -> String {
    "Rerun Python SDK 0.1".to_owned()
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

/// Log a 2D bounding box.
///
/// Optionally give it a label.
/// If no `space` is given, the space name "2D" will be used.
#[pyfunction]
fn log_bbox(
    obj_path: &str,
    left_top: [f32; 2],
    width_height: [f32; 2],
    label: Option<String>,
    space: Option<String>,
) {
    let [x, y] = left_top;
    let [w, h] = width_height;
    let min = [x, y];
    let max = [x + w, y + h];

    let mut sdk = Sdk::global();

    let obj_path = ObjPath::from(obj_path); // TODO(emilk): pass in proper obj path somehow
    sdk.register_type(obj_path.obj_type_path(), ObjectType::BBox2D);

    let time_point = sdk.now();

    sdk.send(LogMsg::DataMsg(DataMsg {
        id: LogId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "bbox".into()),
        data: LoggedData::Single(Data::BBox2D(BBox2D { min, max })),
    }));

    if let Some(label) = label {
        sdk.send(LogMsg::DataMsg(DataMsg {
            id: LogId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "label".into()),
            data: LoggedData::Single(Data::String(label)),
        }));
    }

    let space = space.unwrap_or_else(|| "2D".to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        id: LogId::random(),
        time_point,
        data_path: DataPath::new(obj_path, "space".into()),
        data: LoggedData::Single(Data::Space(space.into())),
    }));
}

/// positions: Nx2 or Nx3 array
/// * `colors.len() == 0`: no colors
/// * `colors.len() == 1`: same color for all points
/// * `colors.len() == positions.len()`: a color per point
///
/// If no `space` is given, the space name "2D" or "3D" will be used,
/// depending on the dimensionality of the data.
#[pyfunction]
fn log_points_rs(
    obj_path: &str,
    positions: numpy::PyReadonlyArray2<'_, f64>,
    colors: numpy::PyReadonlyArray2<'_, u8>,
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

    let root_path = ObjPathBuilder::from(obj_path); // TODO(emilk): pass in proper obj path somehow
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
        let num_colors = match colors.shape() {
            [num_colors, 4] => *num_colors,
            shape => {
                return Err(PyTypeError::new_err(format!(
                    "Expected Nx4 color array; got {shape:?}"
                )));
            }
        };

        match num_colors {
            0 => {}
            1 => {
                let slice = colors.as_slice().unwrap(); // TODO(emilk): Handle non-contiguous arrays
                assert_eq!(slice.len(), 4);
                let color = [slice[0], slice[1], slice[2], slice[3]];

                sdk.send(LogMsg::DataMsg(DataMsg {
                    id: LogId::random(),
                    time_point: time_point.clone(),
                    data_path: DataPath::new(point_path.clone(), "color".into()),
                    data: LoggedData::BatchSplat(Data::Color(color)),
                }));
            }
            n if n == num_pos => {
                let colors: Vec<[u8; 4]> = colors
                    .as_slice()
                    .unwrap()
                    .chunks(4)
                    .into_iter()
                    .map(|chunk| [chunk[0], chunk[1], chunk[2], chunk[3]])
                    .collect();

                sdk.send(LogMsg::DataMsg(DataMsg {
                    id: LogId::random(),
                    time_point: time_point.clone(),
                    data_path: DataPath::new(point_path.clone(), "color".into()),
                    data: LoggedData::Batch {
                        indices: indices.clone(),
                        data: DataVec::Color(colors),
                    },
                }));
            }
            _ => {
                return Err(PyTypeError::new_err(format!("Got {} positions and {} colors. The number of colors must be zero, one, or the same as the number of positions.", positions.len(), colors.len())));
            }
        }
    }

    // TODO(emilk): handle non-contigious arrays
    let pos_data = match dim {
        2 => {
            let data: Vec<[f32; 2]> = positions
                .as_slice()
                .unwrap()
                .chunks(2)
                .into_iter()
                .map(|chunk| [chunk[0] as f32, chunk[1] as f32])
                .collect();
            DataVec::Vec2(data)
        }
        3 => {
            let data: Vec<[f32; 3]> = positions
                .as_slice()
                .unwrap()
                .chunks(3)
                .into_iter()
                .map(|chunk| [chunk[0] as f32, chunk[1] as f32, chunk[2] as f32])
                .collect();
            DataVec::Vec3(data)
        }
        _ => unreachable!(),
    };

    sdk.send(LogMsg::DataMsg(DataMsg {
        id: LogId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(point_path.clone(), "pos".into()),
        data: LoggedData::Batch {
            indices,
            data: pos_data,
        },
    }));

    let space = space.unwrap_or_else(|| if dim == 2 { "2D" } else { "3D" }.to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        id: LogId::random(),
        time_point,
        data_path: DataPath::new(point_path, "space".into()),
        data: LoggedData::BatchSplat(Data::Space(space.into())),
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
) {
    log_tensor(obj_path, img, meter, space);
}

/// If no `space` is given, the space name "2D" will be used.
#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_u16(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, u16>,
    meter: Option<f32>,
    space: Option<String>,
) {
    log_tensor(obj_path, img, meter, space);
}

/// If no `space` is given, the space name "2D" will be used.
#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_tensor_f32(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, f32>,
    meter: Option<f32>,
    space: Option<String>,
) {
    log_tensor(obj_path, img, meter, space);
}

/// If no `space` is given, the space name "2D" will be used.
fn log_tensor<T: TensorDataTypeTrait + numpy::Element + bytemuck::Pod>(
    obj_path: &str,
    img: numpy::PyReadonlyArrayDyn<'_, T>,
    meter: Option<f32>,
    space: Option<String>,
) {
    let mut sdk = Sdk::global();

    let obj_path = ObjPath::from(obj_path); // TODO(emilk): pass in proper obj path somehow
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Image);

    let time_point = sdk.now();

    sdk.send(LogMsg::DataMsg(DataMsg {
        id: LogId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "tensor".into()),
        data: LoggedData::Single(Data::Tensor(to_rerun_tensor(&img))),
    }));

    let space = space.unwrap_or_else(|| "2D".to_owned());
    sdk.send(LogMsg::DataMsg(DataMsg {
        id: LogId::random(),
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.clone(), "space".into()),
        data: LoggedData::Single(Data::Space(space.into())),
    }));

    if let Some(meter) = meter {
        sdk.send(LogMsg::DataMsg(DataMsg {
            id: LogId::random(),
            time_point,
            data_path: DataPath::new(obj_path, "meter".into()),
            data: LoggedData::Single(Data::F32(meter)),
        }));
    }
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
