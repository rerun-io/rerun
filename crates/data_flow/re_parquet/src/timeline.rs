//! Timeline resolution and time-value extraction from parquet schemas.

use arrow::array::{Array, AsArray as _};
use arrow::buffer::ScalarBuffer;
use arrow::datatypes::DataType;
use re_chunk::TimeColumn;
use re_log_types::{TimeType, Timeline, TimelineName};

use crate::config::{IndexColumn, IndexType};
use crate::error::ParquetError;

/// Identifies which column should be used as a timeline and how to scale it.
pub(crate) struct TimelineInfo {
    pub column_index: usize,
    pub timeline: Timeline,

    /// Multiplier to convert raw column values to nanoseconds.
    /// Always 1 for Sequence timelines.
    pub ns_multiplier: i64,
}

/// Resolve explicit [`IndexColumn`] entries to [`TimelineInfo`].
///
/// Returns an error if any named column does not exist in the schema.
pub(crate) fn resolve_explicit_index_columns(
    schema: &arrow::datatypes::Schema,
    columns: &[IndexColumn],
) -> Result<Vec<TimelineInfo>, ParquetError> {
    columns
        .iter()
        .map(|col| {
            let (column_index, _field) = schema
                .fields()
                .iter()
                .enumerate()
                .find(|(_, f)| f.name() == &col.name)
                .ok_or_else(|| ParquetError::index_column_not_found(&col.name))?;

            let time_type = match col.index_type {
                IndexType::Timestamp(_) => TimeType::TimestampNs,
                IndexType::Duration(_) => TimeType::DurationNs,
                IndexType::Sequence => TimeType::Sequence,
            };

            let timeline_name =
                TimelineName::try_new(col.output_name.as_deref().unwrap_or(col.name.as_str()))
                    .map_err(|source| ParquetError::invalid_timeline_name(&col.name, source))?;

            Ok(TimelineInfo {
                column_index,
                timeline: Timeline::new(timeline_name, time_type),
                ns_multiplier: col.index_type.ns_multiplier(),
            })
        })
        .collect()
}

/// Extract i64 time values from a column, scaling raw values to nanoseconds.
///
/// The `ns_multiplier` converts raw values to nanoseconds (1 for ns or sequence,
/// `1_000` for us, etc.). This is determined by the user's `IndexColumn` config,
/// NOT by Arrow schema metadata.
pub(crate) fn extract_time_values(
    array: &dyn Array,
    ns_multiplier: i64,
) -> Option<ScalarBuffer<i64>> {
    // Float columns must scale before rounding: truncating first would collapse
    // sub-unit values (e.g. a `0.033` seconds timestamp) to zero.
    #[expect(clippy::cast_precision_loss)] // multipliers are powers of ten ≤ 1e9, exact in f64
    let scale_float = |v: f64| {
        #[expect(clippy::cast_possible_truncation)]
        {
            (v * ns_multiplier as f64).round() as i64
        }
    };
    // Integer and temporal columns share one i64 scaling pass. A multiplier of 1
    // (ns or sequence) returns the buffer as-is, without copying.
    let scale_i64 = |raw: ScalarBuffer<i64>| -> ScalarBuffer<i64> {
        if ns_multiplier == 1 {
            raw
        } else {
            let scaled: Vec<i64> = raw.iter().map(|&v| v * ns_multiplier).collect();
            ScalarBuffer::from(scaled)
        }
    };

    match array.data_type() {
        DataType::Float64 => {
            let arr = array.as_primitive::<arrow::datatypes::Float64Type>();
            let vals: Vec<i64> = arr.values().iter().map(|&v| scale_float(v)).collect();
            Some(ScalarBuffer::from(vals))
        }

        DataType::Float32 => {
            let arr = array.as_primitive::<arrow::datatypes::Float32Type>();
            let vals: Vec<i64> = arr
                .values()
                .iter()
                .map(|&v| scale_float(f64::from(v)))
                .collect();
            Some(ScalarBuffer::from(vals))
        }

        DataType::Int64 => {
            let arr = array.as_primitive::<arrow::datatypes::Int64Type>();
            Some(scale_i64(arr.values().clone()))
        }

        DataType::Int32 => {
            let arr = array.as_primitive::<arrow::datatypes::Int32Type>();
            let vals: Vec<i64> = arr.values().iter().map(|&v| i64::from(v)).collect();
            Some(scale_i64(ScalarBuffer::from(vals)))
        }

        DataType::Int16 => {
            let arr = array.as_primitive::<arrow::datatypes::Int16Type>();
            let vals: Vec<i64> = arr.values().iter().map(|&v| i64::from(v)).collect();
            Some(scale_i64(ScalarBuffer::from(vals)))
        }

        DataType::UInt64 => {
            let arr = array.as_primitive::<arrow::datatypes::UInt64Type>();
            #[expect(clippy::cast_possible_wrap)]
            let vals: Vec<i64> = arr.values().iter().map(|&v| v as i64).collect();
            Some(scale_i64(ScalarBuffer::from(vals)))
        }

        DataType::UInt32 => {
            let arr = array.as_primitive::<arrow::datatypes::UInt32Type>();
            let vals: Vec<i64> = arr.values().iter().map(|&v| i64::from(v)).collect();
            Some(scale_i64(ScalarBuffer::from(vals)))
        }

        // All Arrow Timestamp and Duration arrays store i64 values internally.
        // We read the raw buffer directly to avoid needing the `compute` feature
        // for `arrow::compute::cast`. Buffer layout is identical across all
        // temporal unit variants (Nanosecond, Microsecond, Millisecond, Second).
        DataType::Timestamp(_, _) | DataType::Duration(_) => {
            let data = array.to_data();
            let buffer = data.buffers()[0].clone();
            let raw = ScalarBuffer::<i64>::new(buffer, data.offset(), data.len());
            Some(scale_i64(raw))
        }

        other => {
            re_log::warn_once!("Cannot use column with type {other:?} as a timeline index");
            None
        }
    }
}

/// Create a fallback sequence timeline using row indices starting at `row_index_offset`.
pub(crate) fn fallback_sequence_timeline(
    row_index_offset: i64,
    num_rows: usize,
) -> re_chunk::external::nohash_hasher::IntMap<re_chunk::TimelineName, TimeColumn> {
    let timeline = Timeline::new("row_index", TimeType::Sequence);
    #[expect(clippy::cast_possible_wrap)]
    let times: Vec<i64> = (row_index_offset..row_index_offset + num_rows as i64).collect();
    let time_column = TimeColumn::new(Some(true), timeline, ScalarBuffer::from(times));
    std::iter::once((*timeline.name(), time_column)).collect()
}
