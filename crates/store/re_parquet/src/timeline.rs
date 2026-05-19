//! Timeline resolution and time-value extraction from parquet schemas.

use arrow::array::{Array, AsArray as _};
use arrow::buffer::ScalarBuffer;
use arrow::datatypes::DataType;
use re_chunk::TimeColumn;
use re_log_types::{TimeType, Timeline};

use crate::config::{IndexColumn, IndexType};
use crate::streaming::ParquetError;

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
                .ok_or_else(|| {
                    ParquetError::from(anyhow::anyhow!(
                        "Index column '{}' not found in parquet schema",
                        col.name
                    ))
                })?;

            let time_type = match col.index_type {
                IndexType::Timestamp(_) => TimeType::TimestampNs,
                IndexType::Duration(_) => TimeType::DurationNs,
                IndexType::Sequence => TimeType::Sequence,
            };

            Ok(TimelineInfo {
                column_index,
                timeline: Timeline::new(col.name.as_str(), time_type),
                ns_multiplier: col.index_type.ns_multiplier(),
            })
        })
        .collect()
}

/// Extract i64 time values from a column, applying the given scaling multiplier.
///
/// The `ns_multiplier` converts raw values to nanoseconds (1 for ns or sequence,
/// `1_000` for us, etc.). This is determined by the user's `IndexColumn` config,
/// NOT by Arrow schema metadata.
pub(crate) fn extract_time_values(
    array: &dyn Array,
    ns_multiplier: i64,
) -> Option<ScalarBuffer<i64>> {
    let raw = extract_raw_i64(array)?;
    if ns_multiplier == 1 {
        Some(raw)
    } else {
        let scaled: Vec<i64> = raw.iter().map(|&v| v * ns_multiplier).collect();
        Some(ScalarBuffer::from(scaled))
    }
}

/// Extract raw i64 values from an Arrow array without any unit conversion.
///
/// For Timestamp/Duration typed arrays, the raw stored i64 is extracted by
/// reading the underlying buffer directly (all Arrow temporal types store i64).
fn extract_raw_i64(array: &dyn Array) -> Option<ScalarBuffer<i64>> {
    match array.data_type() {
        DataType::Int64 => {
            let arr = array.as_primitive::<arrow::datatypes::Int64Type>();
            Some(arr.values().clone())
        }

        DataType::Int32 => {
            let arr = array.as_primitive::<arrow::datatypes::Int32Type>();
            let vals: Vec<i64> = arr.values().iter().map(|&v| i64::from(v)).collect();
            Some(ScalarBuffer::from(vals))
        }

        DataType::Int16 => {
            let arr = array.as_primitive::<arrow::datatypes::Int16Type>();
            let vals: Vec<i64> = arr.values().iter().map(|&v| i64::from(v)).collect();
            Some(ScalarBuffer::from(vals))
        }

        DataType::UInt64 => {
            let arr = array.as_primitive::<arrow::datatypes::UInt64Type>();
            #[expect(clippy::cast_possible_wrap)]
            let vals: Vec<i64> = arr.values().iter().map(|&v| v as i64).collect();
            Some(ScalarBuffer::from(vals))
        }

        DataType::UInt32 => {
            let arr = array.as_primitive::<arrow::datatypes::UInt32Type>();
            let vals: Vec<i64> = arr.values().iter().map(|&v| i64::from(v)).collect();
            Some(ScalarBuffer::from(vals))
        }

        DataType::Float64 => {
            let arr = array.as_primitive::<arrow::datatypes::Float64Type>();
            #[expect(clippy::cast_possible_truncation)]
            let vals: Vec<i64> = arr.values().iter().map(|&v| v as i64).collect();
            Some(ScalarBuffer::from(vals))
        }

        DataType::Float32 => {
            let arr = array.as_primitive::<arrow::datatypes::Float32Type>();
            #[expect(clippy::cast_possible_truncation)]
            let vals: Vec<i64> = arr.values().iter().map(|&v| v as i64).collect();
            Some(ScalarBuffer::from(vals))
        }

        // All Arrow Timestamp and Duration arrays store i64 values internally.
        // We read the raw buffer directly to avoid needing the `compute` feature
        // for `arrow::compute::cast`. Buffer layout is identical across all
        // temporal unit variants (Nanosecond, Microsecond, Millisecond, Second).
        DataType::Timestamp(_, _) | DataType::Duration(_) => {
            let data = array.to_data();
            let buffer = data.buffers()[0].clone();
            let values = ScalarBuffer::<i64>::new(buffer, data.offset(), data.len());
            Some(values)
        }

        other => {
            re_log::warn_once!("Cannot use column with type {other:?} as a timeline index");
            None
        }
    }
}

/// Create a fallback sequence timeline using row indices starting at `offset`.
pub(crate) fn fallback_sequence_timeline(
    offset: i64,
    num_rows: usize,
) -> re_chunk::external::nohash_hasher::IntMap<re_chunk::TimelineName, TimeColumn> {
    let timeline = Timeline::new("row_index", TimeType::Sequence);
    #[expect(clippy::cast_possible_wrap)]
    let times: Vec<i64> = (offset..offset + num_rows as i64).collect();
    let time_column = TimeColumn::new(Some(true), timeline, ScalarBuffer::from(times));
    std::iter::once((*timeline.name(), time_column)).collect()
}
