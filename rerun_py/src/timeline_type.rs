use arrow::datatypes::DataType;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use re_log_types::TimeType;

const SEQUENCE: &str = "sequence";
const DURATION_NS: &str = "duration_ns";
const TIMESTAMP_NS: &str = "timestamp_ns";
const DURATION: &str = "duration";
const TIMESTAMP: &str = "timestamp";

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(timeline_datatype, m)?)?;
    m.add_function(wrap_pyfunction!(timeline_type_from_datatype, m)?)?;
    Ok(())
}

/// The short name of a timeline type, as used by `rerun.catalog.TimelineKind`.
pub fn timeline_type_name(time_type: TimeType) -> &'static str {
    match time_type {
        TimeType::Sequence => SEQUENCE,
        TimeType::DurationNs => DURATION,
        TimeType::TimestampNs => TIMESTAMP,
    }
}

/// The Arrow datatype of index columns of a timeline type.
#[pyfunction]
fn timeline_datatype(timeline_type: &str) -> PyResult<PyArrowType<DataType>> {
    Ok(PyArrowType(parse_timeline_type(timeline_type)?.datatype()))
}

/// The short name (`sequence`, `duration`, or `timestamp`) of the timeline type an index column of this Arrow datatype holds, if any.
#[pyfunction]
#[expect(clippy::needless_pass_by_value)] // PyO3 requires owned arguments for #[pyfunction]
fn timeline_type_from_datatype(datatype: PyArrowType<DataType>) -> Option<&'static str> {
    TimeType::from_arrow_datatype(&datatype.0).map(timeline_type_name)
}

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
