use pyo3::prelude::*;
use re_log_types::{Data, DataMsg, DataPath, LogId, LogMsg, TimePoint, TimeValue};

use crate::sdk::Sdk;

/// The python module is called "rerun_sdk".
#[pymodule]
fn rerun_sdk(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    m.add_function(wrap_pyfunction!(info, m)?)?;
    m.add_function(wrap_pyfunction!(log_point, m)?)?;
    Ok(())
}

#[pyfunction]
fn info() -> String {
    "Rerun Python SDK".to_owned()
}

#[pyfunction]
fn log_point(name: &str, x: f32, y: f32) {
    let mut sdk = Sdk::global();
    let mut time_point = TimePoint::default();
    time_point.0.insert(
        "log_time".into(),
        TimeValue::Time(re_log_types::Time::now()),
    );
    let data = Data::Vec2([x, y]);
    let data_msg = DataMsg {
        id: LogId::random(),
        time_point,
        data_path: DataPath::new(name.into(), "pos".into()),
        data: re_log_types::LoggedData::Single(data),
    };
    let log_msg = LogMsg::DataMsg(data_msg);
    sdk.send(&log_msg);
}
