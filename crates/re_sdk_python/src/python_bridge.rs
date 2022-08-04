use pyo3::prelude::*;
use re_log_types::{
    Data, DataMsg, DataPath, LogId, LogMsg, ObjPath, ObjectType, TimePoint, TimeValue,
};

use crate::sdk::Sdk;

/// The python module is called "rerun_sdk".
#[pymodule]
fn rerun_sdk(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    m.add_function(wrap_pyfunction!(info, m)?)?;
    m.add_function(wrap_pyfunction!(log_point, m)?)?;
    m.add_function(wrap_pyfunction!(connect_remote, m)?)?;
    m.add_function(wrap_pyfunction!(buffer, m)?)?;
    m.add_function(wrap_pyfunction!(show_and_quit, m)?)?;
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
fn show_and_quit() {
    let mut sdk = Sdk::global();
    if sdk.is_buffered() {
        let log_messages = sdk.drain_log_messages();
        drop(sdk);
        let (tx, rx) = std::sync::mpsc::channel();
        for log_msg in log_messages {
            tx.send(log_msg).ok();
        }
        // TODO: don't quit! Solve https://github.com/emilk/egui/issues/1223
        re_viewer::run_native_viewer(rx);
    } else {
        tracing::error!("Can't show the log messages of Rerurn: it was configured to send the data to a server!");
    }
}

#[pyfunction]
fn log_point(name: &str, x: f32, y: f32) {
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
    let log_msg = LogMsg::DataMsg(data_msg);
    sdk.send(&log_msg);
}

#[allow(clippy::needless_pass_by_value)]
#[pyfunction]
fn log_image(name: &str, img: numpy::PyReadonlyArrayDyn<'_, u8>) -> PyResult<()> {
    let mut sdk = Sdk::global();

    let obj_path = ObjPath::from(name); // TODO(emilk): pass in proper obj path somehow
    sdk.register_type(obj_path.obj_type_path(), ObjectType::Image);
    let data_path = DataPath::new(obj_path, "image".into());

    let image = to_rerun_image(&img)?;

    let data = Data::Image(image);
    let data_msg = DataMsg {
        id: LogId::random(),
        time_point: time_point(),
        data_path,
        data: re_log_types::LoggedData::Single(data),
    };
    let log_msg = LogMsg::DataMsg(data_msg);
    sdk.send(&log_msg);

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

fn to_rerun_image(img: &numpy::PyReadonlyArrayDyn<'_, u8>) -> PyResult<re_log_types::Image> {
    let shape = img.shape();

    let [w, h, depth] = match shape.len() {
        // NOTE: opencv/numpy uses "height x width" convention
        2 => [shape[1], shape[0], 1],
        3 => [shape[1], shape[0], shape[2]],
        _ => {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Expected image of dim 2 or 3. Got image of shape {shape:?}"
            )));
        }
    };

    let size = [w as u32, h as u32];
    let data = img.to_owned_array().into_raw_vec();

    if data.len() != w * h * depth {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Got image of shape {shape:?} (product = {}), but data length is {}",
            w * h * depth,
            data.len()
        )));
    }

    let format = match depth {
        1 => re_log_types::ImageFormat::Luminance8,
        3 => re_log_types::ImageFormat::Rgb8,
        4 => re_log_types::ImageFormat::Rgba8,
        _ => {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Expected depth to be one of 1,3,4. Got image of shape {shape:?}",
            )));
        }
    };

    Ok(re_log_types::Image { size, format, data })
}
