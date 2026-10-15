//! Human-readable formatting shared with the viewer, so Python reprs read the same as the UI.

use pyo3::prelude::*;
use pyo3::types::PyModule;
use re_log_types::{DateVisibility, TimeInt, TimestampFormat};

use crate::timeline_type::parse_timeline_type;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(format_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(format_uint, m)?)?;
    m.add_function(wrap_pyfunction!(format_index_value, m)?)?;
    Ok(())
}

/// Format a byte count with binary units, e.g. `12.1 KiB`.
#[pyfunction]
fn format_bytes(num_bytes: f64) -> String {
    re_format::format_bytes(num_bytes)
}

/// Format an unsigned integer with thousands separators.
#[pyfunction]
fn format_uint(number: u64) -> String {
    re_format::format_uint(number)
}

/// Format one index value of a timeline, as the viewer does, with timestamps in UTC and always dated.
#[pyfunction]
fn format_index_value(timeline_type: &str, value: i64) -> PyResult<String> {
    let time_type = parse_timeline_type(timeline_type)?;
    let timestamp_format = TimestampFormat::utc().with_date_visibility(DateVisibility::ShowDate);
    Ok(time_type.format(TimeInt::saturated_temporal_i64(value), timestamp_format))
}
