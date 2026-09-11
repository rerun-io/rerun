use pyo3::{PyResult, exceptions::PyValueError};
use re_log_types::TimeType;

const SEQUENCE: &str = "sequence";
const DURATION_NS: &str = "duration_ns";
const TIMESTAMP_NS: &str = "timestamp_ns";
const DURATION: &str = "duration";
const TIMESTAMP: &str = "timestamp";

pub fn parse_temporal_timeline_type(value: &str) -> PyResult<TimeType> {
    match parse_timeline_type(value)? {
        TimeType::Sequence => Err(PyValueError::new_err(
            "Invalid timeline_type: \"sequence\". Expected \"duration_ns\" \
               or \"timestamp_ns\" (\"duration\" and \"timestamp\" are also accepted)",
        )),
        temporal_type => Ok(temporal_type),
    }
}

pub fn parse_timeline_type(value: &str) -> PyResult<TimeType> {
    match value {
        SEQUENCE => Ok(re_log_types::TimeType::Sequence),
        DURATION_NS | DURATION => Ok(re_log_types::TimeType::DurationNs),
        TIMESTAMP_NS | TIMESTAMP => Ok(re_log_types::TimeType::TimestampNs),
        _ => Err(PyValueError::new_err(format!(
            "Invalid timeline_type: {value:?}. Expected \"sequence\", \
               \"duration_ns\", or \"timestamp_ns\" \
               (\"duration\" and \"timestamp\" are also accepted)"
        ))),
    }
}
