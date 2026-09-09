//! Fetch planning: packs the server's per-segment `chunk_info` batches into
//! target-sized `FetchChunks` requests.
//!
//! Every input batch must hold the rows of exactly one segment, and the input
//! must be in `SegmentId` ASC order. `SegmentStreamExec::execute` establishes
//! both: `group_chunk_infos_by_segment_id` buckets rows into a `BTreeMap` keyed
//! by `SegmentId`, then the caller concatenates each bucket down to a *single*
//! batch before handing the `Vec` over. The within-segment sort below is
//! per-batch, so if that concatenation ever stops collapsing a segment to one
//! batch, a segment would become several independently sorted runs laid
//! end to end — plan order would silently stop matching time order. That is
//! what [`extract_segment_id`]'s debug assert guards.
//!
//! Given that, the planner's output order is load-bearing twice over:
//!
//! * across segments, batches follow the caller-provided global segment order
//!   (`SegmentId` ASC), which is what keeps the `[rerun_segment_id ASC, ..]`
//!   output-ordering claim honest;
//! * within a segment, rows are sorted by the `{timeline}:start` value
//!   (chunk `time_min` on the query's filtered index), with static chunks and
//!   null-`:start` rows first. The v2 pipeline's plan-cursor watermark derives
//!   each segment's safe horizon from plan position, which is only sound if
//!   plan order equals `time_min` order.

use std::collections::HashSet;

use arrow::array::RecordBatch;
use re_dataframe::TimelineName;
use re_dataframe::external::re_chunk::TimeColumn;
use re_protos::cloud::v1alpha1::ext::QueryDatasetDataframe;
use re_redap_client::{ApiError, ApiResult};
use re_types_core::SegmentId;

/// Target batch size in bytes for grouping segments together in requests.
/// This reduces the number of round-trips while keeping memory usage bounded (as long
/// as the concurrency is also bounded).
pub const TARGET_BATCH_SIZE_BYTES: usize = 8 * 1024 * 1024; // 8 MB

pub type BatchingResult = (Vec<RecordBatch>, Vec<SegmentId>);

/// Extract segment ID from a `chunk_info` `RecordBatch`. Each `chunk_info` batch contains
/// chunks *for a single segment*, hence we can just take the first row's `segment_id`. See the
/// module docs for who establishes that invariant and what silently breaks without it.
fn extract_segment_id(origin: &re_uri::Origin, chunk_info: &RecordBatch) -> ApiResult<SegmentId> {
    let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
        .extract(chunk_info)
        .map_err(|err| ApiError::internal_quiver(origin, err))?;

    // `re_log::debug_assert!` short-circuits on `cfg!(debug_assertions)`, so the scan does not
    // run in release builds.
    let first = segment_ids.value(0);
    re_log::debug_assert!(
        (&segment_ids).into_iter().all(|id| id == first),
        "`chunk_info` batch mixes segments; both the per-segment byte accounting and the \
         within-segment `:start` sort assume one segment per batch"
    );

    Ok(segment_ids.value_owned(0))
}

/// Extract chunk sizes (`chunk_byte_len` values) from a `chunk_info` `RecordBatch`.
fn extract_chunk_sizes(
    origin: &re_uri::Origin,
    chunk_info: &RecordBatch,
) -> ApiResult<quiver::Column<u64>> {
    QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN
        .extract(chunk_info)
        .map_err(|err| ApiError::internal_quiver(origin, err))
}

/// Read a `{timeline}:start` column as raw `i64`s plus its null mask.
///
/// `:start` carries the timeline's `time_min` for each chunk. The OSS server types the column
/// `Int64` (see `QueryDatasetResponse::field_timeline_start`); other servers may emit
/// `TimestampNanosecondArray` / `Time64NanosecondArray` / `DurationNanosecondArray` matching the
/// timeline's native dtype. All four are i64 under the hood, so read through
/// [`TimeColumn::read_nullable_array`] rather than downcasting to any one of them.
///
/// Callers that must not fail on a malformed column read the array directly and discard the
/// error instead of going through here.
pub fn read_start_column(
    origin: &re_uri::Origin,
    start_col: &dyn arrow::array::Array,
    col_name: &str,
) -> ApiResult<(
    arrow::buffer::ScalarBuffer<i64>,
    Option<arrow::buffer::NullBuffer>,
)> {
    TimeColumn::read_nullable_array(start_col).map_err(|err| {
        ApiError::internal(
            origin,
            format!("`{col_name}` column has unsupported type: {err}"),
        )
    })
}

/// Per-row sort key for within-segment plan order.
///
/// Time-less rows (static chunk, or null `:start`) map to `i64::MIN` so they sort before every
/// temporal value. That sentinel cannot collide with a real `time_min`: `i64::MIN` is reserved as
/// `TimeInt`'s static marker, so its smallest temporal value is `i64::MIN + 1`, and `TimeInt`'s own
/// `as_i64` maps static to `i64::MIN` in exactly the same way.
///
/// Placing time-less chunks first means a plan-position cursor gates all emission on them, closing
/// the gap where a late-arriving static chunk misses rows that already emitted.
fn row_start_key(
    is_static: bool,
    start_values: &[i64],
    start_nulls: Option<&arrow::buffer::NullBuffer>,
    row: usize,
) -> i64 {
    if is_static || start_nulls.is_some_and(|n| n.is_null(row)) {
        i64::MIN
    } else {
        start_values[row]
    }
}

/// Stable-sort a single-segment `chunk_info` batch by `{timeline}:start`
/// ascending, static and null-`:start` rows first.
///
/// No-ops (returning the batch unchanged) when the query has no temporal
/// index or the server did not attach the `:start` column — those cases fall
/// back to completion-only emission and have no time order to respect.
fn sort_chunk_info_rows_by_start(
    origin: &re_uri::Origin,
    chunk_info: RecordBatch,
    start_col_name: Option<&str>,
) -> ApiResult<RecordBatch> {
    let Some(col_name) = start_col_name else {
        return Ok(chunk_info);
    };
    let Some(start_col) = chunk_info.column_by_name(col_name) else {
        return Ok(chunk_info);
    };

    let (start_values, start_nulls) = read_start_column(origin, start_col.as_ref(), col_name)?;
    let is_statics = QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC
        .extract(&chunk_info)
        .map_err(|err| ApiError::internal_quiver(origin, err))?;

    let key = |row: usize| {
        row_start_key(
            is_statics.value(row),
            &start_values,
            start_nulls.as_ref(),
            row,
        )
    };

    // Server row order is arbitrary but often already time-ordered; skip both the key buffer and
    // the permutation copy when it is.
    if (0..chunk_info.num_rows()).is_sorted_by_key(key) {
        return Ok(chunk_info);
    }

    // Materializing the keys keeps the comparator to a branchless integer compare and evaluates
    // each key once, rather than the `O(n log n)` key calls a `sort_by_key` would make. The row
    // index is part of the key, so the unstable sort still yields the stable permutation.
    let mut keyed: Vec<(i64, usize)> = (0..chunk_info.num_rows())
        .map(|row| (key(row), row))
        .collect();
    keyed.sort_unstable();
    let indices: Vec<usize> = keyed.iter().map(|&(_, row)| row).collect();

    re_arrow_util::take_record_batch(&chunk_info, &indices).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            None,
            err,
            "sorting chunk-info rows by :start",
        )
    })
}

/// Groups `chunk_infos` into batches targeting the specified size, with special handling
/// for segments larger than the target size (which get split). Batches smaller than `target_size`
/// are merged together to reduce the number of requests.
///
/// Within each segment, rows are first sorted by `{filtered_index_timeline}:start`
/// (static / null-`:start` rows first) — see the module docs for why plan
/// order must equal time order.
///
/// Returns (batches, `segment_order`) where:
/// - batches: list of merged `RecordBatch`es, each representing a `target_size` request
/// - `segment_order`: Original order of segments for preserving segment order
#[tracing::instrument(
    level = "info",
    skip_all,
    fields(
        num_chunk_infos = chunk_infos.len(),
        target_size_bytes,
        output_batches,
        byte_target_flushes,
        segment_limit_flushes,
        large_segment_batches,
        end_of_input_batches,
    )
)]
pub fn create_request_batches_with_segment_limit(
    origin: &re_uri::Origin,
    chunk_infos: Vec<RecordBatch>,
    target_size_bytes: u64,
    segment_limit: usize,
    filtered_index_timeline: Option<TimelineName>,
) -> ApiResult<BatchingResult> {
    re_tracing::profile_function!();
    let merge_err = |err: arrow::error::ArrowError, ctx: &'static str| {
        ApiError::deserialization_with_source(origin, None, err, ctx)
    };

    let start_col_name = filtered_index_timeline.map(|t| format!("{t}:start"));

    let mut request_batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut current_batch_size = 0u64;
    let mut current_batch_segments: HashSet<SegmentId> = HashSet::new();
    let mut segment_order = Vec::new();
    let mut segment_seen = HashSet::new();
    let mut segments_in_wave = 0usize;
    let mut byte_target_flushes = 0usize;
    let mut segment_limit_flushes = 0usize;
    let mut large_segment_batches = 0usize;
    let mut end_of_input_batches = 0usize;

    for chunk_info in chunk_infos {
        let chunk_info =
            sort_chunk_info_rows_by_start(origin, chunk_info, start_col_name.as_deref())?;
        let segment_id = extract_segment_id(origin, &chunk_info)?;
        let chunk_sizes = extract_chunk_sizes(origin, &chunk_info)?;
        let segment_size: u64 = chunk_sizes.iter().sum();

        let is_new_segment = segment_seen.insert(segment_id.clone());
        if is_new_segment && segments_in_wave == segment_limit {
            if !current_batch.is_empty() {
                segment_limit_flushes += 1;
                let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
                    .map_err(|err| merge_err(err, "merging segment-wave boundary batch"))?;
                request_batches.push(merged_batch);
                current_batch = Vec::new();
                current_batch_size = 0;
                current_batch_segments.clear();
            }
            segments_in_wave = 0;
        }
        if is_new_segment {
            segment_order.push(segment_id.clone());
            segments_in_wave += 1;
        }

        // Check if this chunk_info would push the current batch past
        // either the byte target OR the segment-count cap. The
        // segment-count check matters when small segments would
        // otherwise merge more than `segment_limit` distinct segments into a
        // single fetch: the resulting reservation
        // could never satisfy the segment-count gate in
        // `PipelineBudget::try_admit` and would deadlock.
        let adds_new_segment = !current_batch_segments.contains(&segment_id);
        let would_exceed_size = current_batch_size + segment_size > target_size_bytes;
        let would_exceed_segments =
            adds_new_segment && current_batch_segments.len() >= segment_limit;
        if !current_batch.is_empty() && (would_exceed_size || would_exceed_segments) {
            if would_exceed_segments {
                segment_limit_flushes += 1;
            } else {
                byte_target_flushes += 1;
            }
            // Merge current batch and add to results
            let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
                .map_err(|err| merge_err(err, "merging chunk-info batches"))?;
            request_batches.push(merged_batch);
            current_batch = Vec::new();
            current_batch_size = 0;
            current_batch_segments.clear();
        }

        // Split the large segment into multiple requests
        if segment_size > target_size_bytes {
            // If current batch is not empty, merge and send it first
            if !current_batch.is_empty() {
                byte_target_flushes += 1;
                let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
                    .map_err(|err| merge_err(err, "merging chunk-info batches"))?;
                request_batches.push(merged_batch);
                current_batch = Vec::new();
                current_batch_size = 0;
                current_batch_segments.clear();
            }

            let split_batches = split_large_segments(
                &segment_id,
                &chunk_info,
                target_size_bytes,
                &chunk_sizes,
                segment_size,
            );
            large_segment_batches += split_batches.len();

            // Split batches are already individual RecordBatches, add them directly
            for split_batch in split_batches {
                request_batches.push(split_batch);
            }
        } else {
            current_batch.push(chunk_info);
            current_batch_size += segment_size;
            current_batch_segments.insert(segment_id);
        }
    }

    // Don't forget to merge the last batch
    if !current_batch.is_empty() {
        let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
            .map_err(|err| merge_err(err, "merging final chunk-info batch"))?;
        request_batches.push(merged_batch);
        end_of_input_batches += 1;
    }

    re_log::debug_assert_eq!(
        request_batches.len(),
        byte_target_flushes + segment_limit_flushes + large_segment_batches + end_of_input_batches,
        "every planned batch must have exactly one flush reason"
    );

    let span = tracing::Span::current();
    span.record("output_batches", request_batches.len());
    span.record("byte_target_flushes", byte_target_flushes);
    span.record("segment_limit_flushes", segment_limit_flushes);
    span.record("large_segment_batches", large_segment_batches);
    span.record("end_of_input_batches", end_of_input_batches);

    tracing::debug!(
        "Batching complete: {} segments → {} batches (target_size={}KB)",
        segment_order.len(),
        request_batches.len(),
        target_size_bytes / 1024
    );

    Ok((request_batches, segment_order))
}

/// Split segment larger than target size into multiple smaller requests. Each request will contain
/// a subset of the chunks from the original segment, targeting approximately the desired size.
///
/// Consumes rows in order, so a batch that was sorted by `:start` produces
/// sub-batches that respect the same time order.
///
/// Each sub-batch is therefore a *contiguous* row range, which is why this slices rather than
/// gathers: `RecordBatch::slice` only adjusts per-column offsets, so a segment is never copied
/// here. The sub-batches share the parent's buffers until the last of them has been fetched —
/// still strictly less resident memory than materializing a copy per sub-batch.
///
/// Sub-batch membership is what range merging in `fetch_batch_via_direct_urls` operates on: it
/// can only coalesce byte-adjacent chunks that landed in the same request, and only within a
/// single layer URL. Cutting on time boundaries rather than the server's row order therefore
/// changes HTTP range-merge efficiency, not just ordering.
fn split_large_segments(
    segment_id: &SegmentId,
    chunk_info: &RecordBatch,
    target_size: u64,
    chunk_sizes: &quiver::Column<u64>,
    segment_size: u64,
) -> Vec<RecordBatch> {
    re_tracing::profile_function!();

    let mut result_batches = Vec::new();
    let mut current_start = 0usize;
    let mut current_len = 0usize;
    let mut current_size = 0u64;

    for row_idx in 0..chunk_info.num_rows() {
        let chunk_size = chunk_sizes[row_idx];

        // Always include at least one chunk per batch (even if it exceeds target)
        if current_len == 0 || current_size + chunk_size <= target_size {
            current_len += 1;
            current_size += chunk_size;
        } else {
            result_batches.push(chunk_info.slice(current_start, current_len));

            // Start new batch with current chunk
            current_start = row_idx;
            current_len = 1;
            current_size = chunk_size;
        }
    }

    // Don't forget the last batch
    if current_len > 0 {
        result_batches.push(chunk_info.slice(current_start, current_len));
    }

    tracing::debug!(
        "Split large segment '{}' ({}) into {} requests",
        segment_id,
        re_format::format_bytes(segment_size as _),
        result_batches.len()
    );

    result_batches
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use arrow::array::{
        Array as _, ArrayRef, BooleanArray, DurationNanosecondArray, FixedSizeBinaryBuilder,
        Float64Array, Int64Array, RecordBatchOptions, StringArray, Time64NanosecondArray,
        TimestampNanosecondArray, UInt64Array,
    };
    use arrow::datatypes::{Field, Schema};

    use super::*;

    /// Chunk-info batch with `:start` and `chunk_is_static` columns.
    /// `starts[i] = None` means a null `:start` value.
    fn chunk_info_with_starts(
        segment_id: &str,
        timeline: &str,
        starts: &[Option<i64>],
        statics: &[bool],
    ) -> RecordBatch {
        chunk_info_rows(&vec![segment_id; starts.len()], timeline, starts, statics)
    }

    /// Same, but with a per-row `segment_id` — only the invariant-violation test needs this.
    fn chunk_info_rows(
        row_segment_ids: &[&str],
        timeline: &str,
        starts: &[Option<i64>],
        statics: &[bool],
    ) -> RecordBatch {
        chunk_info_with_start_array(
            row_segment_ids,
            timeline,
            Arc::new(Int64Array::from(starts.to_vec())),
            statics,
        )
    }

    /// Same, but with a caller-supplied `:start` array, so the dtype tests can hand over the
    /// non-`Int64` encodings a non-OSS server may use.
    fn chunk_info_with_start_array(
        row_segment_ids: &[&str],
        timeline: &str,
        start_col: ArrayRef,
        statics: &[bool],
    ) -> RecordBatch {
        assert_eq!(start_col.len(), statics.len());
        assert_eq!(start_col.len(), row_segment_ids.len());
        let num_chunks = statics.len();
        let start_type = start_col.data_type().clone();

        let segment_ids = StringArray::from(row_segment_ids.to_vec());
        let sizes = UInt64Array::from(vec![10u64; num_chunks]);
        let mut chunk_id_builder = FixedSizeBinaryBuilder::with_capacity(num_chunks, 16);
        for i in 0..num_chunks {
            let mut id_bytes = [0u8; 16];
            id_bytes[0..4].copy_from_slice(&(i as u32).to_le_bytes());
            chunk_id_builder.append_value(id_bytes).unwrap();
        }
        let chunk_ids = chunk_id_builder.finish();
        let static_col = BooleanArray::from(statics.to_vec());

        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field(),
                Field::new(
                    QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN_NAME,
                    arrow::datatypes::DataType::UInt64,
                    false,
                ),
                QueryDatasetDataframe::COLUMN_CHUNK_ID.arrow_field(),
                QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC.arrow_field(),
                Field::new(format!("{timeline}:start"), start_type, true),
            ],
            HashMap::default(),
        ));

        RecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(segment_ids),
                Arc::new(sizes),
                Arc::new(chunk_ids),
                Arc::new(static_col),
                start_col,
            ],
            &RecordBatchOptions::new().with_row_count(Some(num_chunks)),
        )
        .unwrap()
    }

    fn starts_of(batch: &RecordBatch, timeline: &str) -> Vec<Option<i64>> {
        let col = batch
            .column_by_name(&format!("{timeline}:start"))
            .unwrap()
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        (0..col.len())
            .map(|i| (!col.is_null(i)).then(|| col.value(i)))
            .collect()
    }

    /// Reads `:start` back as raw `i64`s whatever the column's dtype, mirroring how production
    /// reads it. `starts_of` can't be used for the non-`Int64` cases — it downcasts.
    fn raw_starts_of(batch: &RecordBatch, timeline: &str) -> Vec<Option<i64>> {
        let col = batch
            .column_by_name(&format!("{timeline}:start"))
            .expect("`:start` column");
        let (values, nulls) =
            TimeColumn::read_nullable_array(col.as_ref()).expect("supported `:start` dtype");
        (0..values.len())
            .map(|i| (!nulls.as_ref().is_some_and(|n| n.is_null(i))).then(|| values[i]))
            .collect()
    }

    /// Per-row `(segment_id, :start)` pairs, for asserting the planner's full
    /// output order: segments in caller order, rows ascending within each.
    fn segment_starts_of(batch: &RecordBatch, timeline: &str) -> Vec<(SegmentId, Option<i64>)> {
        let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(batch)
            .unwrap();
        starts_of(batch, timeline)
            .into_iter()
            .enumerate()
            .map(|(i, start)| (segment_ids.value_owned(i), start))
            .collect()
    }

    #[test]
    fn within_segment_rows_sort_by_start() {
        let chunk_info = chunk_info_with_starts(
            "seg1",
            "t",
            &[Some(30), Some(10), Some(20)],
            &[false, false, false],
        );

        let (batches, _order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info],
            1_000_000,
            usize::MAX,
            Some(TimelineName::from("t")),
        )
        .unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(
            starts_of(&batches[0], "t"),
            vec![Some(10), Some(20), Some(30)]
        );
    }

    #[test]
    fn static_and_null_start_rows_sort_first() {
        let chunk_info = chunk_info_with_starts(
            "seg1",
            "t",
            &[Some(20), None, Some(10), Some(999)],
            &[false, false, false, true], // the `999` row is static
        );

        let (batches, _order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info],
            1_000_000,
            usize::MAX,
            Some(TimelineName::from("t")),
        )
        .unwrap();

        assert_eq!(batches.len(), 1);
        // Null-`:start` and static rows first (stable: input order among
        // themselves), then temporal ascending.
        assert_eq!(
            starts_of(&batches[0], "t"),
            vec![None, Some(999), Some(10), Some(20)]
        );
    }

    #[test]
    fn sort_is_stable_for_equal_starts() {
        let chunk_info = chunk_info_with_starts(
            "seg1",
            "t",
            &[Some(10), Some(5), Some(10)],
            &[false, false, false],
        );

        let (batches, _order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info],
            1_000_000,
            usize::MAX,
            Some(TimelineName::from("t")),
        )
        .unwrap();

        // The two `10` rows keep their relative input order: chunk ids
        // row0 then row2.
        let ids = batches[0]
            .column_by_name(QueryDatasetDataframe::COLUMN_CHUNK_ID_NAME)
            .unwrap();
        let ids = ids
            .as_any()
            .downcast_ref::<arrow::array::FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(ids.value(0)[0], 1); // the `5` row (input row 1)
        assert_eq!(ids.value(1)[0], 0);
        assert_eq!(ids.value(2)[0], 2);
    }

    #[test]
    fn sort_spans_large_segment_split_batches() {
        // Segment larger than target: split into sub-batches; `:start`
        // must be monotone across the batch boundary.
        let chunk_info = chunk_info_with_starts(
            "seg1",
            "t",
            &[Some(40), Some(10), Some(30), Some(20)],
            &[false; 4],
        );

        let (batches, _order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info],
            20, // 10 bytes per chunk → 2 chunks per sub-batch
            usize::MAX,
            Some(TimelineName::from("t")),
        )
        .unwrap();

        assert!(batches.len() > 1, "expected a large-segment split");
        let all_starts: Vec<Option<i64>> = batches.iter().flat_map(|b| starts_of(b, "t")).collect();
        assert_eq!(
            all_starts,
            vec![Some(10), Some(20), Some(30), Some(40)],
            "`:start` must be monotone across sub-batch boundaries"
        );
    }

    /// The two order guarantees are independent and must both survive packing:
    /// segments stay in caller order (`SegmentId` ASC), rows sort by `:start`
    /// *within* a segment only. `seg3`'s starts are all lower than `seg1`'s, so
    /// a sort that leaked across the segment boundary would reorder segments.
    ///
    /// `segment_limit = 2` forces a segment-wave flush mid-input, exercising the
    /// merge path (`concat_polymorphic_batches`) rather than one batch per segment.
    #[test]
    fn sort_survives_multi_segment_packing() {
        let chunk_infos = vec![
            chunk_info_with_starts("seg1", "t", &[Some(30), Some(10), Some(20)], &[false; 3]),
            chunk_info_with_starts("seg2", "t", &[Some(100), Some(300), Some(200)], &[false; 3]),
            chunk_info_with_starts("seg3", "t", &[Some(7), Some(5), Some(6)], &[false; 3]),
        ];

        let (batches, segment_order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            chunk_infos,
            1_000_000, // byte target never binds; only the segment limit flushes
            2,
            Some(TimelineName::from("t")),
        )
        .unwrap();

        assert_eq!(
            segment_order,
            vec![
                SegmentId::from("seg1"),
                SegmentId::from("seg2"),
                SegmentId::from("seg3")
            ]
        );
        // seg1 + seg2 merged into the wave-boundary flush, seg3 into the final one.
        assert_eq!(batches.len(), 2);

        let all: Vec<_> = batches
            .iter()
            .flat_map(|b| segment_starts_of(b, "t"))
            .collect();
        assert_eq!(
            all,
            vec![
                (SegmentId::from("seg1"), Some(10)),
                (SegmentId::from("seg1"), Some(20)),
                (SegmentId::from("seg1"), Some(30)),
                (SegmentId::from("seg2"), Some(100)),
                (SegmentId::from("seg2"), Some(200)),
                (SegmentId::from("seg2"), Some(300)),
                (SegmentId::from("seg3"), Some(5)),
                (SegmentId::from("seg3"), Some(6)),
                (SegmentId::from("seg3"), Some(7)),
            ]
        );
    }

    /// Same guarantees when the three flush reasons interleave: `seg1` rides the
    /// byte-target flush, `seg2` exceeds the target and takes the split path,
    /// `seg3` lands in the end-of-input batch.
    #[test]
    fn sort_survives_mixed_packing_with_large_segment() {
        let chunk_infos = vec![
            // 20 bytes: exactly the target, so it packs rather than splits.
            chunk_info_with_starts("seg1", "t", &[Some(40), Some(20)], &[false; 2]),
            // 40 bytes: over target → split into 2 sub-batches of 2 chunks.
            chunk_info_with_starts(
                "seg2",
                "t",
                &[Some(4), Some(2), Some(3), Some(1)],
                &[false; 4],
            ),
            chunk_info_with_starts("seg3", "t", &[Some(900), Some(800)], &[false; 2]),
        ];

        let (batches, segment_order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            chunk_infos,
            20, // 10 bytes per chunk → 2 chunks per request
            usize::MAX,
            Some(TimelineName::from("t")),
        )
        .unwrap();

        assert_eq!(
            segment_order,
            vec![
                SegmentId::from("seg1"),
                SegmentId::from("seg2"),
                SegmentId::from("seg3")
            ]
        );
        assert_eq!(batches.len(), 4, "seg1, seg2 × 2 sub-batches, seg3");

        let all: Vec<_> = batches
            .iter()
            .flat_map(|b| segment_starts_of(b, "t"))
            .collect();
        assert_eq!(
            all,
            vec![
                (SegmentId::from("seg1"), Some(20)),
                (SegmentId::from("seg1"), Some(40)),
                (SegmentId::from("seg2"), Some(1)),
                (SegmentId::from("seg2"), Some(2)),
                (SegmentId::from("seg2"), Some(3)),
                (SegmentId::from("seg2"), Some(4)),
                (SegmentId::from("seg3"), Some(800)),
                (SegmentId::from("seg3"), Some(900)),
            ]
        );
    }

    /// The OSS server types `:start` as `Int64`, but another server may emit the timeline's
    /// native dtype. All of these are `i64` underneath, which is why `read_start_column` goes
    /// through `TimeColumn::read_nullable_array` instead of downcasting to one of them — sorting
    /// has to work identically for each.
    #[test]
    fn start_column_sorts_for_every_supported_dtype() {
        let starts = [Some(30), None, Some(10), Some(20)];
        let statics = [false; 4];
        let expected = vec![None, Some(10), Some(20), Some(30)];

        let raw: Vec<Option<i64>> = starts.to_vec();
        let columns: Vec<(&str, ArrayRef)> = vec![
            ("Int64", Arc::new(Int64Array::from(raw.clone()))),
            (
                "Timestamp(ns)",
                Arc::new(TimestampNanosecondArray::from(raw.clone())),
            ),
            (
                "Time64(ns)",
                Arc::new(Time64NanosecondArray::from(raw.clone())),
            ),
            (
                "Duration(ns)",
                Arc::new(DurationNanosecondArray::from(raw.clone())),
            ),
        ];

        for (label, start_col) in columns {
            let chunk_info =
                chunk_info_with_start_array(&["seg1"; 4], "t", start_col, statics.as_slice());

            let (batches, _order) = create_request_batches_with_segment_limit(
                &re_uri::Origin::test(),
                vec![chunk_info],
                1_000_000,
                usize::MAX,
                Some(TimelineName::from("t")),
            )
            .unwrap_or_else(|err| panic!("{label} `:start` must be sortable: {err}"));

            assert_eq!(batches.len(), 1, "{label}");
            assert_eq!(raw_starts_of(&batches[0], "t"), expected, "{label}");
        }
    }

    /// The deliberate hard error: a `:start` column that is not an `i64` under the hood fails the
    /// whole query rather than silently skipping the sort. Reaching this needs a server that both
    /// types `:start` exotically and disagrees with itself across batches — see the discussion on
    /// the PR.
    #[test]
    fn unsupported_start_dtype_is_an_error() {
        let chunk_info = chunk_info_with_start_array(
            &["seg1"; 2],
            "t",
            Arc::new(Float64Array::from(vec![30.0, 10.0])),
            &[false, false],
        );

        let err = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info],
            1_000_000,
            usize::MAX,
            Some(TimelineName::from("t")),
        )
        .expect_err("unsupported `:start` dtype must not be silently ignored");

        assert!(
            err.to_string().contains("unsupported type"),
            "unexpected error: {err}"
        );
    }

    /// A batch spanning two segments would leave the within-segment sort
    /// silently interleaving them; the caller's one-batch-per-segment collapse
    /// must keep that from happening.
    #[test]
    #[should_panic(expected = "batch mixes segments")]
    fn mixed_segment_batch_trips_the_debug_assert() {
        let chunk_info = chunk_info_rows(
            &["seg1", "seg2"],
            "t",
            &[Some(30), Some(10)],
            &[false, false],
        );

        _ = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info],
            1_000_000,
            usize::MAX,
            Some(TimelineName::from("t")),
        );
    }

    #[test]
    fn no_timeline_or_missing_column_is_a_noop() {
        let chunk_info =
            chunk_info_with_starts("seg1", "t", &[Some(30), Some(10)], &[false, false]);

        // No timeline: untouched.
        let (batches, _order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info.clone()],
            1_000_000,
            usize::MAX,
            None,
        )
        .unwrap();
        assert_eq!(starts_of(&batches[0], "t"), vec![Some(30), Some(10)]);

        // Timeline set but no matching `:start` column: untouched.
        let (batches, _order) = create_request_batches_with_segment_limit(
            &re_uri::Origin::test(),
            vec![chunk_info],
            1_000_000,
            usize::MAX,
            Some(TimelineName::from("other")),
        )
        .unwrap();
        assert_eq!(starts_of(&batches[0], "t"), vec![Some(30), Some(10)]);
    }
}
