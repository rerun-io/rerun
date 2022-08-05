#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pufunction] need to be by value

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use re_log_types::*;

use crate::sdk::Sdk;

/// The python module is called "rerun_sdk".
#[pymodule]
fn rerun_sdk(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    m.add_function(wrap_pyfunction!(info, m)?)?;
    m.add_function(wrap_pyfunction!(connect_remote, m)?)?;

    #[cfg(feature = "re_viewer")]
    {
        m.add_function(wrap_pyfunction!(buffer, m)?)?;
        m.add_function(wrap_pyfunction!(show, m)?)?;
    }

    m.add_function(wrap_pyfunction!(log_point2d, m)?)?;
    m.add_function(wrap_pyfunction!(log_points_rs, m)?)?;
    m.add_function(wrap_pyfunction!(log_image, m)?)?;
    Ok(())
}

#[pyfunction]
fn info() -> String {
    "Rerun Python SDK 0.1".to_owned()
}

#[pyfunction]
fn connect_remote() {
    Sdk::global().configure_remote();
}

/// Call this first to tell Rerun to buffer the log data so it can be shown
/// later with `show()`.
#[cfg(feature = "re_viewer")]
#[pyfunction]
fn buffer() {
    Sdk::global().configure_buffered();
}

/// Show the buffered log data.
#[cfg(feature = "re_viewer")]
#[pyfunction]
fn show() {
    let mut sdk = Sdk::global();
    if sdk.is_buffered() {
        let log_messages = sdk.drain_log_messages();
        drop(sdk);
        let (tx, rx) = std::sync::mpsc::channel();
        for log_msg in log_messages {
            tx.send(log_msg).ok();
        }
        re_viewer::run_native_viewer(rx);
    } else {
        tracing::error!("Can't show the log messages of Rerurn: it was configured to send the data to a server!");
    }
}

#[pyfunction]
fn log_point2d(name: &str, x: f32, y: f32) {
    let mut sdk = Sdk::global();

    let obj_path = ObjPath::from(name); // TODO(emilk): pass in proper obj path somehow
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Point2D);
    let data_path = DataPath::new(obj_path, "pos".into());

    let data = Data::Vec2([x, y]);
    let data_msg = DataMsg {
        id: LogId::random(),
        time_point: time_point(),
        data_path,
        data: re_log_types::LoggedData::Single(data),
    };
    sdk.send(LogMsg::DataMsg(data_msg));
}

/// positions: Nx2 or Nx3 array
/// * `colors.len() == 0`: no colors
/// * `colors.len() == 1`: same color for all points
/// * `colors.len() == positions.len()`: a color per point
#[pyfunction]
fn log_points_rs(
    name: &str,
    positions: numpy::PyReadonlyArray2<'_, f64>,
    colors: numpy::PyReadonlyArray2<'_, u8>,
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

    let root_path = ObjPathBuilder::from(name); // TODO(emilk): pass in proper obj path somehow
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

    let time_point = time_point();

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
                    data: re_log_types::LoggedData::BatchSplat(Data::Color(color)),
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
                    data: re_log_types::LoggedData::Batch {
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
        time_point,
        data_path: DataPath::new(point_path, "pos".into()),
        data: re_log_types::LoggedData::Batch {
            indices,
            data: pos_data,
        },
    }));

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_image(name: &str, img: numpy::PyReadonlyArrayDyn<'_, u8>) -> PyResult<()> {
    match img.shape() {
        // NOTE: opencv/numpy uses "height x width" convention
        [_, _] | [_, _, 1 | 3 | 4] => {}
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Expected image of dimension of 2 or 3 with a depth of 1 (gray), 3 (RGB) or 4 (RGBA). Got image of shape {:?}", img.shape()
            )));
        }
    };

    // ----------------

    let mut sdk = Sdk::global();

    let obj_path = ObjPath::from(name); // TODO(emilk): pass in proper obj path somehow
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Image);
    let data_path = DataPath::new(obj_path, "image".into());

    let data = Data::Tensor(to_rerun_tensor(&img));
    let data_msg = DataMsg {
        id: LogId::random(),
        time_point: time_point(),
        data_path,
        data: re_log_types::LoggedData::Single(data),
    };
    let log_msg = LogMsg::DataMsg(data_msg);
    sdk.send(log_msg);

    Ok(())
}

fn time_point() -> TimePoint {
    let mut time_point = TimePoint::default();
    time_point.0.insert(
        "log_time".into(),
        TimeValue::Time(re_log_types::Time::now()),
    );
    time_point
}

fn to_rerun_tensor(img: &numpy::PyReadonlyArrayDyn<'_, u8>) -> re_log_types::Tensor {
    re_log_types::Tensor {
        shape: img.shape().iter().map(|&d| d as u64).collect(),
        dtype: TensorDataType::U8,
        data: TensorData::Dense(img.to_owned_array().into_raw_vec()),
    }
}
