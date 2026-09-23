//! The immutable fetch plan: everything the pipeline needs to know about a
//! partition's work, computed once before anything runs.
//!
//! The plan is built on the shared packing core (`fetch_plan.rs`), which
//! already emits batches in `[global segment order, CursorKey order within
//! segment]`. On top of that, this module derives each chunk's cursor key and
//! estimated decoded bytes once, so the plan itself can serve as the
//! watermark source: each segment's safe horizon is a function of which
//! planned chunks have been delivered, and no downstream consumer re-scans
//! the chunk-info columns.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arrow::array::RecordBatch;
use re_dataframe::TimelineName;
use re_dataframe::external::re_chunk::ChunkId;
use re_log_types::TimeInt;
use re_protos::cloud::v1alpha1::ext::QueryDatasetDataframe;
use re_redap_client::{ApiError, ApiResult};
use re_types_core::SegmentId;

use crate::chunk_fetcher::split_batch_by_direct_url;
use crate::dataframe_query_common::IndexValuesMap;
use crate::dataframe_query_provider::fetch_plan::{
    create_request_batches_with_segment_limit, read_start_column,
};

/// Sort/cursor key for within-segment plan order.
///
/// `PreTime` (static chunk, or null `{timeline}:start`) sorts before every
/// temporal value, mirroring the planner's row sort. Placing time-less
/// chunks first means the plan cursor holds the safe horizon at
/// "nothing emittable" until they are delivered — closing the gap where a
/// late-arriving static chunk would be missing from rows that already
/// emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CursorKey {
    /// Static chunk, or temporal chunk with a null `{timeline}:start`.
    PreTime,

    /// Chunk `time_min` on the query's filtered index.
    Time(TimeInt),
}

/// How a segment's rows are emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SegmentEmitMode {
    /// Incremental horizon emit + GC, driven by the plan cursor.
    Watermark,

    /// Buffer the whole segment, emit once at completion. Selected when:
    /// - `using_index_values` applies to this segment (the explicit value
    ///   list overrides `filtered_index_range` inside `QueryHandle`, so
    ///   incremental emit would replay rows), or
    /// - the query has no temporal `filtered_index` (static-only), or
    /// - the `{timeline}:start` column is absent from this segment's
    ///   chunk-info batch (old server), leaving no basis for a horizon.
    BufferAll,
}

/// Which transport a planned batch is fetched over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FetchTransport {
    /// Presigned direct-URL HTTP range reads.
    Direct,

    /// `FetchChunks` gRPC.
    Grpc,
}

/// One transport-homogeneous request within a [`PlannedBatch`].
pub(crate) struct PlannedRequest {
    /// The chunk-info rows for exactly this request.
    pub request: RecordBatch,
    pub transport: FetchTransport,

    /// Sum of the estimates of every chunk this request covers.
    pub est_bytes: u64,
}

/// One packed unit of plan work: ~`TARGET_BATCH_SIZE_BYTES` of chunk-info
/// rows, covered by a single issuance-window lease.
///
/// A batch carries one request per transport its rows use, so one or two.
/// Both occupy a single plan position, which is what the ordering invariant
/// on [`FetchPlan::batches`] rests on: split across two positions, a
/// direct-URL request for a later segment could be issued ahead of a gRPC
/// request for an earlier one.
pub(crate) struct PlannedBatch {
    /// One entry per transport present, `Direct` before `Grpc`. Issue them
    /// together: they share this batch's window lease, so neither half can
    /// starve the other.
    pub requests: Vec<PlannedRequest>,

    /// Sum of every request's `est_bytes` — the issuance-window acquisition
    /// for the whole batch.
    pub est_bytes: u64,
}

/// Everything the pipeline knows about one segment, straight from the plan.
pub(crate) struct PlannedSegment {
    pub segment_id: SegmentId,
    pub mode: SegmentEmitMode,

    /// Exact-ID lookup for routing deliveries: chunk → (key, est bytes).
    ///
    /// The segment's whole plan, in one shape: chunk count, cursor seed and
    /// byte total all derive from it. Relies on `ChunkId` being unique within
    /// a segment, which [`build_fetch_plan`] enforces.
    pub chunk_meta: HashMap<ChunkId, (CursorKey, u64)>,
}

impl PlannedSegment {
    /// Total chunks planned for this segment; delivery of the last one is
    /// segment completion.
    pub fn expected_chunks(&self) -> usize {
        self.chunk_meta.len()
    }

    /// Multiset of cursor keys, seeding the segment's plan cursor. A
    /// multiset (not a sequence) so the watermark stays correct under
    /// out-of-order delivery (`buffer_unordered` fallback).
    pub fn cursor_keys(&self) -> BTreeMap<CursorKey, u32> {
        let mut keys = BTreeMap::new();
        for &(key, _est_bytes) in self.chunk_meta.values() {
            *keys.entry(key).or_insert(0) += 1;
        }
        keys
    }

    /// Sum of the segment's chunk estimates.
    pub fn est_bytes(&self) -> u64 {
        self.chunk_meta
            .values()
            .map(|&(_key, est_bytes)| est_bytes)
            .sum()
    }
}

/// The per-partition fetch plan: immutable, shared, computed before any IO.
///
/// `batches` is in issuance order == emission order: global segment order
/// (`SegmentId` ASC), `CursorKey` ASC within each segment. That identity is
/// load-bearing for deadlock freedom: the oldest outstanding fetch always
/// belongs to the segment the consumer needs next, so head-of-line emission
/// can never be waiting on a fetch that is queued behind it. The transport
/// split lives *inside* a batch ([`PlannedBatch::requests`]) precisely so it
/// cannot perturb this order.
pub(crate) struct FetchPlan {
    /// Global segment order (`SegmentId` ASC) == emission order. Shared, so
    /// per-segment state can hold onto its entry rather than copy it.
    pub segments: Vec<Arc<PlannedSegment>>,
    pub batches: Vec<Arc<PlannedBatch>>,

    /// Index into [`Self::segments`] by id, for routing deliveries.
    segment_indices: HashMap<SegmentId, usize>,
}

impl FetchPlan {
    pub fn segment_index(&self, segment_id: &SegmentId) -> Option<usize> {
        self.segment_indices.get(segment_id).copied()
    }
}

/// Build the fetch plan for one partition.
///
/// `chunk_infos` is one batch per segment, `SegmentId` ASC (the
/// `group_chunk_infos_by_segment_id` invariant). Segments absent from a
/// present `index_values` map are dropped here: they cannot produce output,
/// so fetching them is pure waste.
pub(crate) fn build_fetch_plan(
    origin: &re_uri::Origin,
    chunk_infos: Vec<RecordBatch>,
    filtered_index: Option<TimelineName>,
    index_values: &IndexValuesMap,
    target_size_bytes: u64,
) -> ApiResult<FetchPlan> {
    re_tracing::profile_function!();

    let start_col_name = filtered_index.map(|t| format!("{t}:start"));

    // Pre-pass over the per-segment inputs: apply the index_values filter
    // and decide each segment's emit mode (the `:start` schema check must
    // run per segment, before packing merges batches).
    let mut kept: Vec<(SegmentId, RecordBatch)> = Vec::with_capacity(chunk_infos.len());
    let mut modes: HashMap<SegmentId, SegmentEmitMode> = HashMap::new();
    for chunk_info in chunk_infos {
        if chunk_info.num_rows() == 0 {
            continue;
        }
        let segment_id =
            crate::dataframe_query_provider::fetch_plan::extract_segment_id(origin, &chunk_info)?;

        let uses_index_values = match index_values.as_ref() {
            Some(iv) => {
                if !iv.contains_key(&segment_id) {
                    // Cannot produce output; don't fetch it at all.
                    continue;
                }
                true
            }
            None => false,
        };

        let has_start = start_col_name
            .as_deref()
            .is_some_and(|name| chunk_info.column_by_name(name).is_some());

        let mode = if uses_index_values || !has_start {
            SegmentEmitMode::BufferAll
        } else {
            SegmentEmitMode::Watermark
        };
        // Each segment must arrive as exactly one batch. The packer's
        // `:start` sort runs per batch, so a segment split across two would
        // become two independently sorted runs laid end to end: plan order
        // would stop matching time order, the horizon would stay pinned by a
        // late-planned early chunk, and a segment larger than the issuance
        // window could wedge the pipeline — delivered chunks unable to emit
        // or GC while the pinning chunk waits behind a full window. A hard
        // error here turns that silent hang into a visible failure.
        if modes.insert(segment_id.clone(), mode).is_some() {
            return Err(ApiError::internal(
                origin,
                format!(
                    "segment {segment_id} spans multiple chunk_info batches; callers must \
                     concatenate each segment's batches before building the plan"
                ),
            ));
        }
        kept.push((segment_id, chunk_info));
    }

    // The packer preserves input order, and plan order is emission order,
    // declared to the exec node as `SegmentId` ASC. Sorting here establishes
    // that by construction rather than trusting the caller: a downstream
    // `SortPreservingMergeExec` does not validate its inputs' sortedness, so a
    // violation is silently wrong output, not a slow query. Input is already
    // sorted in the normal case, which this costs a linear scan to confirm.
    kept.sort_by(|(a, _), (b, _)| a.cmp(b));
    let kept: Vec<RecordBatch> = kept.into_iter().map(|(_, chunk_info)| chunk_info).collect();

    // Packing (with the within-segment CursorKey sort inside). No segment
    // cap: lookahead is byte-bounded by the issuance window instead.
    let (request_batches, segment_order) = create_request_batches_with_segment_limit(
        origin,
        kept,
        target_size_bytes,
        usize::MAX,
        filtered_index,
    )?;

    let segment_indices: HashMap<SegmentId, usize> = segment_order
        .iter()
        .enumerate()
        .map(|(idx, id)| (id.clone(), idx))
        .collect();
    let mut segments: Vec<PlannedSegment> = segment_order
        .into_iter()
        .map(|segment_id| {
            let mode = modes
                .get(&segment_id)
                .copied()
                .unwrap_or(SegmentEmitMode::BufferAll);
            PlannedSegment {
                segment_id,
                mode,
                chunk_meta: HashMap::new(),
            }
        })
        .collect();

    // Postcondition of the sort above: the packer is expected to preserve its
    // input order, and the exec node's declared `SegmentId` ASC output
    // ordering depends on it.
    re_log::debug_assert!(
        segments.is_sorted_by(|a, b| a.segment_id <= b.segment_id),
        "planned segments must be in SegmentId ASC order; got {:?}",
        segments
            .iter()
            .map(|s| s.segment_id.as_ref())
            .collect::<Vec<_>>(),
    );

    // One plan entry per packed batch. The transport split happens *within*
    // the entry, so a batch keeps its single position in plan order.
    let mut batches: Vec<Arc<PlannedBatch>> = Vec::with_capacity(request_batches.len());
    for batch in request_batches {
        let (direct, grpc) = split_batch_by_direct_url(&batch);

        let mut requests = Vec::with_capacity(2);
        let mut est_bytes = 0;
        for (request, transport) in [
            (direct, FetchTransport::Direct),
            (grpc, FetchTransport::Grpc),
        ] {
            let Some(request) = request else { continue };
            let planned = plan_request(
                origin,
                request,
                transport,
                start_col_name.as_deref(),
                &segment_indices,
                &mut segments,
            )?;
            est_bytes += planned.est_bytes;
            requests.push(planned);
        }

        if !requests.is_empty() {
            batches.push(Arc::new(PlannedBatch {
                requests,
                est_bytes,
            }));
        }
    }

    Ok(FetchPlan {
        segments: segments.into_iter().map(Arc::new).collect(),
        batches,
        segment_indices,
    })
}

/// Derive one transport-homogeneous request's byte estimate, populating the
/// owning segments' `chunk_meta` as a side effect.
///
/// Each row of a packed batch reaches this exactly once — the transport split
/// partitions the rows.
fn plan_request(
    origin: &re_uri::Origin,
    request: RecordBatch,
    transport: FetchTransport,
    start_col_name: Option<&str>,
    segment_indices: &HashMap<SegmentId, usize>,
    segments: &mut [PlannedSegment],
) -> ApiResult<PlannedRequest> {
    let chunk_ids = QueryDatasetDataframe::COLUMN_CHUNK_ID
        .extract(&request)
        .map_err(|err| ApiError::internal_quiver(origin, err))?;
    let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
        .extract(&request)
        .map_err(|err| ApiError::internal_quiver(origin, err))?;
    let is_statics = QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC
        .extract(&request)
        .map_err(|err| ApiError::internal_quiver(origin, err))?;
    let compressed_sizes = QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN
        .extract(&request)
        .map_err(|err| ApiError::internal_quiver(origin, err))?;
    let uncompressed_sizes = QueryDatasetDataframe::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED
        .extract(&request)
        .ok();

    // `:start`, same dtype tolerance as everywhere else (Int64 / Timestamp /
    // Time64 / Duration are all i64 underneath).
    let starts = start_col_name
        .and_then(|name| request.column_by_name(name).map(|col| (name, col)))
        .map(|(name, col)| read_start_column(origin, col.as_ref(), name))
        .transpose()?;

    let mut est_total = 0u64;
    for row in 0..request.num_rows() {
        let segment_id = segment_ids.value_owned(row);
        let Some(&segment_idx) = segment_indices.get(&segment_id) else {
            // Cannot happen by construction (the packer produced both).
            return Err(ApiError::internal(
                origin,
                format!("planned batch references unknown segment {segment_id}"),
            ));
        };

        let key = match &starts {
            Some((values, nulls)) => {
                if is_statics.value(row) || nulls.as_ref().is_some_and(|n| n.is_null(row)) {
                    CursorKey::PreTime
                } else {
                    CursorKey::Time(TimeInt::saturated_temporal_i64(values[row]))
                }
            }
            None => CursorKey::PreTime,
        };

        let est_bytes = uncompressed_sizes
            .as_ref()
            .and_then(|col| col.value_owned(row))
            .filter(|&v| v > 0)
            .unwrap_or_else(|| compressed_sizes[row]);

        let chunk_id = chunk_ids.value_owned(row);

        let segment = &mut segments[segment_idx];
        if segment
            .chunk_meta
            .insert(chunk_id, (key, est_bytes))
            .is_some()
        {
            return Err(ApiError::internal(
                origin,
                format!("chunk {chunk_id} planned twice within segment {segment_id}"),
            ));
        }

        est_total += est_bytes;
    }

    Ok(PlannedRequest {
        request,
        transport,
        est_bytes: est_total,
    })
}

#[cfg(test)]
mod tests {
    use arrow::array::{
        BooleanArray, FixedSizeBinaryBuilder, Int64Array, RecordBatchOptions, StringArray,
        UInt64Array,
    };
    use arrow::datatypes::{Field, Schema};

    use super::*;

    /// Chunk-info batch with `:start`, `chunk_is_static`, and uncompressed
    /// sizes. `starts[i] = None` → null `:start`.
    fn chunk_info(
        segment_id: &str,
        timeline: Option<&str>,
        starts: &[Option<i64>],
        statics: &[bool],
        uncompressed: &[Option<u64>],
    ) -> RecordBatch {
        assert_eq!(starts.len(), statics.len());
        assert_eq!(starts.len(), uncompressed.len());
        let n = starts.len();

        let segment_ids = StringArray::from(vec![segment_id; n]);
        let sizes = UInt64Array::from(vec![10u64; n]);
        let mut chunk_id_builder = FixedSizeBinaryBuilder::with_capacity(n, 16);
        for i in 0..n {
            let mut id_bytes = [0u8; 16];
            id_bytes[0..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
            id_bytes[8..16].copy_from_slice(
                segment_id
                    .as_bytes()
                    .first_chunk::<8>()
                    .map_or(&[0u8; 8], |c| c),
            );
            chunk_id_builder.append_value(id_bytes).unwrap();
        }
        let chunk_ids = chunk_id_builder.finish();
        let static_col = BooleanArray::from(statics.to_vec());
        let uncompressed_col = UInt64Array::from(uncompressed.to_vec());

        let mut fields = vec![
            QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field(),
            Field::new(
                QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN_NAME,
                arrow::datatypes::DataType::UInt64,
                false,
            ),
            QueryDatasetDataframe::COLUMN_CHUNK_ID.arrow_field(),
            QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC.arrow_field(),
            Field::new(
                QueryDatasetDataframe::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED_NAME,
                arrow::datatypes::DataType::UInt64,
                true,
            ),
        ];
        let mut columns: Vec<arrow::array::ArrayRef> = vec![
            Arc::new(segment_ids),
            Arc::new(sizes),
            Arc::new(chunk_ids),
            Arc::new(static_col),
            Arc::new(uncompressed_col),
        ];
        if let Some(timeline) = timeline {
            fields.push(Field::new(
                format!("{timeline}:start"),
                arrow::datatypes::DataType::Int64,
                true,
            ));
            columns.push(Arc::new(Int64Array::from(starts.to_vec())));
        }

        let schema = Arc::new(Schema::new_with_metadata(fields, Default::default()));
        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::new().with_row_count(Some(n)),
        )
        .unwrap()
    }

    fn t(v: i64) -> CursorKey {
        CursorKey::Time(TimeInt::new_temporal(v))
    }

    /// Plan order of one request's rows as `(segment index, cursor key)`,
    /// read back off the batch that will actually be issued and resolved
    /// through the plan's own lookups.
    fn planned_order(plan: &FetchPlan, request: &PlannedRequest) -> Vec<(usize, CursorKey)> {
        let chunk_ids = QueryDatasetDataframe::COLUMN_CHUNK_ID
            .extract(&request.request)
            .unwrap();
        let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(&request.request)
            .unwrap();

        (0..request.request.num_rows())
            .map(|row| {
                let segment_idx = plan.segment_index(&segment_ids.value_owned(row)).unwrap();
                let (key, _est_bytes) =
                    plan.segments[segment_idx].chunk_meta[&chunk_ids.value_owned(row)];
                (segment_idx, key)
            })
            .collect()
    }

    #[test]
    fn cursor_key_pretime_sorts_first() {
        assert!(CursorKey::PreTime < t(i64::MIN + 1));
        assert!(t(1) < t(2));
    }

    #[test]
    fn plan_derives_keys_counts_and_estimates() {
        let infos = vec![
            chunk_info(
                "seg-a",
                Some("frame"),
                &[Some(20), Some(10), None],
                &[false, false, false],
                &[Some(100), Some(200), Some(300)],
            ),
            chunk_info(
                "seg-b",
                Some("frame"),
                &[Some(5)],
                &[true], // static row → PreTime despite a start value
                &[None], // no uncompressed estimate → fall back to chunk_byte_len (10)
            ),
        ];

        let plan = build_fetch_plan(
            &re_uri::Origin::test(),
            infos,
            Some(TimelineName::from("frame")),
            &None,
            1_000_000,
        )
        .unwrap();

        assert_eq!(plan.segments.len(), 2);
        let a = &plan.segments[0];
        assert_eq!(a.segment_id.as_ref(), "seg-a");
        assert_eq!(a.mode, SegmentEmitMode::Watermark);
        assert_eq!(a.expected_chunks(), 3);
        assert_eq!(
            a.cursor_keys(),
            [(CursorKey::PreTime, 1), (t(10), 1), (t(20), 1)]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        );
        assert_eq!(a.est_bytes(), 600);

        let b = &plan.segments[1];
        assert_eq!(b.mode, SegmentEmitMode::Watermark);
        assert_eq!(
            b.cursor_keys(),
            std::iter::once((CursorKey::PreTime, 1)).collect::<BTreeMap<_, _>>()
        );
        assert_eq!(b.est_bytes(), 10);

        // All rows landed in one gRPC request (no direct-url column), in
        // plan order: seg-a PreTime, seg-a t10, seg-a t20, seg-b PreTime.
        assert_eq!(plan.batches.len(), 1);
        let batch = &plan.batches[0];
        assert_eq!(batch.requests.len(), 1);
        let request = &batch.requests[0];
        assert_eq!(request.transport, FetchTransport::Grpc);
        assert_eq!(
            planned_order(&plan, request),
            vec![
                (0, CursorKey::PreTime),
                (0, t(10)),
                (0, t(20)),
                (1, CursorKey::PreTime),
            ]
        );
        assert_eq!(batch.est_bytes, 610);
    }

    /// Chunk-info batch carrying a nullable direct-URL column, so the
    /// transport split has something to split on. `urls[i] == None` → that
    /// row is fetched over gRPC.
    fn chunk_info_with_urls(
        segment_id: &str,
        starts: &[Option<i64>],
        urls: &[Option<&str>],
    ) -> RecordBatch {
        let n = starts.len();
        let base = chunk_info(
            segment_id,
            Some("frame"),
            starts,
            &vec![false; n],
            &vec![Some(10u64); n],
        );

        let mut fields: Vec<Field> = base
            .schema()
            .fields()
            .iter()
            .map(|field| field.as_ref().clone())
            .collect();
        let mut columns = base.columns().to_vec();
        fields.push(Field::new(
            QueryDatasetDataframe::COLUMN_RERUN_LAYER_DIRECT_URL_NAME,
            arrow::datatypes::DataType::Utf8,
            true,
        ));
        columns.push(Arc::new(StringArray::from(urls.to_vec())));

        RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(fields, Default::default())),
            columns,
            &RecordBatchOptions::new().with_row_count(Some(n)),
        )
        .unwrap()
    }

    /// A packed batch whose rows span both transports stays one plan
    /// position. Two positions would issue seg-b's direct-URL request ahead
    /// of seg-a's gRPC one, inverting the order the deadlock-freedom
    /// argument rests on.
    #[test]
    fn mixed_transport_batch_keeps_one_plan_position() {
        let infos = vec![
            chunk_info_with_urls("seg-a", &[Some(10), Some(20)], &[None, Some("https://x/1")]),
            chunk_info_with_urls("seg-b", &[Some(30)], &[Some("https://x/2")]),
        ];

        let plan = build_fetch_plan(
            &re_uri::Origin::test(),
            infos,
            Some(TimelineName::from("frame")),
            &None,
            1_000_000, // big target: both segments pack into one request batch
        )
        .unwrap();

        assert_eq!(
            plan.batches.len(),
            1,
            "one packed batch must stay one plan position",
        );

        let batch = &plan.batches[0];
        let transports: Vec<FetchTransport> = batch.requests.iter().map(|r| r.transport).collect();
        assert_eq!(
            transports,
            vec![FetchTransport::Direct, FetchTransport::Grpc]
        );
        assert_eq!(
            batch.est_bytes, 30,
            "one lease covers every row of the batch, both transports",
        );

        // Every planned row is accounted for exactly once across the split.
        let mut planned: Vec<(usize, CursorKey)> = batch
            .requests
            .iter()
            .flat_map(|request| planned_order(&plan, request))
            .collect();
        planned.sort();
        assert_eq!(planned, vec![(0, t(10)), (0, t(20)), (1, t(30))]);

        // Per-segment aggregates counted each row once, not once per request.
        assert_eq!(plan.segments[0].expected_chunks(), 2);
        assert_eq!(plan.segments[1].expected_chunks(), 1);
    }

    #[test]
    fn missing_start_column_yields_buffer_all() {
        let infos = vec![chunk_info(
            "seg-a",
            None, // no `:start` column at all
            &[None, None],
            &[false, false],
            &[Some(1), Some(2)],
        )];

        let plan = build_fetch_plan(
            &re_uri::Origin::test(),
            infos,
            Some(TimelineName::from("frame")),
            &None,
            1_000_000,
        )
        .unwrap();

        assert_eq!(plan.segments[0].mode, SegmentEmitMode::BufferAll);
        // Every chunk keys as PreTime.
        assert_eq!(
            plan.segments[0].cursor_keys(),
            std::iter::once((CursorKey::PreTime, 2)).collect::<BTreeMap<_, _>>()
        );
    }

    #[test]
    fn static_only_query_yields_buffer_all() {
        let infos = vec![chunk_info(
            "seg-a",
            Some("frame"),
            &[Some(10)],
            &[false],
            &[Some(1)],
        )];

        let plan =
            build_fetch_plan(&re_uri::Origin::test(), infos, None, &None, 1_000_000).unwrap();
        assert_eq!(plan.segments[0].mode, SegmentEmitMode::BufferAll);
    }

    #[test]
    fn index_values_filter_drops_and_marks_segments() {
        use re_dataframe::IndexValue;
        use std::collections::{BTreeMap as StdBTreeMap, BTreeSet};

        let infos = vec![
            chunk_info("seg-a", Some("frame"), &[Some(1)], &[false], &[Some(1)]),
            chunk_info("seg-b", Some("frame"), &[Some(1)], &[false], &[Some(1)]),
        ];

        let index_values: StdBTreeMap<SegmentId, BTreeSet<IndexValue>> = std::iter::once((
            SegmentId::from("seg-a"),
            BTreeSet::from([IndexValue::new_temporal(1)]),
        ))
        .collect();
        let index_values = Some(Arc::new(index_values));

        let plan = build_fetch_plan(
            &re_uri::Origin::test(),
            infos,
            Some(TimelineName::from("frame")),
            &index_values,
            1_000_000,
        )
        .unwrap();

        // seg-b dropped entirely; seg-a present in BufferAll mode.
        assert_eq!(plan.segments.len(), 1);
        assert_eq!(plan.segments[0].segment_id.as_ref(), "seg-a");
        assert_eq!(plan.segments[0].mode, SegmentEmitMode::BufferAll);
    }

    #[test]
    fn segment_index_lookup() {
        let infos = vec![
            chunk_info("a", Some("frame"), &[Some(1)], &[false], &[Some(1)]),
            chunk_info("b", Some("frame"), &[Some(1)], &[false], &[Some(1)]),
        ];
        let plan = build_fetch_plan(
            &re_uri::Origin::test(),
            infos,
            Some(TimelineName::from("frame")),
            &None,
            1_000_000,
        )
        .unwrap();
        assert_eq!(plan.segment_index(&SegmentId::from("a")), Some(0));
        assert_eq!(plan.segment_index(&SegmentId::from("b")), Some(1));
        assert_eq!(plan.segment_index(&SegmentId::from("zz")), None);
    }

    /// A segment split across two `chunk_info` batches must be caught at plan
    /// time. Each half is single-segment, so `extract_segment_id`'s assert
    /// sees nothing wrong — but the `:start` sort is per batch, so the
    /// segment's plan order would come out as two sorted runs end to end
    /// (`30, 40, 10, 20`) rather than ascending. A hard error (not a debug
    /// assert): the release-mode failure is a pipeline hang, not bad output.
    #[test]
    fn segment_split_across_batches_errors() {
        let infos = vec![
            chunk_info(
                "seg-a",
                Some("frame"),
                &[Some(30), Some(40)],
                &[false, false],
                &[Some(1), Some(1)],
            ),
            chunk_info(
                "seg-a",
                Some("frame"),
                &[Some(10), Some(20)],
                &[false, false],
                &[Some(1), Some(1)],
            ),
        ];
        let Err(err) = build_fetch_plan(
            &re_uri::Origin::test(),
            infos,
            Some(TimelineName::from("frame")),
            &None,
            1_000_000,
        ) else {
            panic!("a segment split across batches must fail plan building");
        };
        assert!(
            err.to_string()
                .contains("spans multiple chunk_info batches"),
            "unexpected error: {err}",
        );
    }

    /// `chunk_meta` is the segment's only plan record, so the chunk count and
    /// the cursor seed both derive from `ChunkId` uniqueness. A repeat would
    /// collapse into one entry and under-count the segment forever, so it has
    /// to fail loudly at plan time.
    #[test]
    fn duplicate_chunk_id_within_a_segment_errors() {
        let one = chunk_info("seg-a", Some("frame"), &[Some(10)], &[false], &[Some(1)]);
        let doubled =
            arrow::compute::concat_batches(&one.schema(), std::iter::once(&one).cycle().take(2))
                .unwrap();

        let Err(err) = build_fetch_plan(
            &re_uri::Origin::test(),
            vec![doubled],
            Some(TimelineName::from("frame")),
            &None,
            1_000_000,
        ) else {
            panic!("a repeated chunk id must fail plan building");
        };
        assert!(
            err.to_string().contains("planned twice within segment"),
            "unexpected error: {err}",
        );
    }

    /// Plan order is declared to the exec node as `SegmentId` ASC output
    /// ordering, and nothing downstream validates it — so the plan sorts
    /// rather than relying on the caller's input order.
    #[test]
    fn unsorted_input_is_sorted_at_plan_time() {
        let infos = vec![
            chunk_info("seg-c", Some("frame"), &[Some(1)], &[false], &[Some(1)]),
            chunk_info("seg-a", Some("frame"), &[Some(1)], &[false], &[Some(1)]),
            chunk_info("seg-b", Some("frame"), &[Some(1)], &[false], &[Some(1)]),
        ];
        let plan = build_fetch_plan(
            &re_uri::Origin::test(),
            infos,
            Some(TimelineName::from("frame")),
            &None,
            1_000_000,
        )
        .unwrap();

        let ids: Vec<&str> = plan
            .segments
            .iter()
            .map(|s| s.segment_id.as_ref())
            .collect();
        assert_eq!(ids, vec!["seg-a", "seg-b", "seg-c"]);

        // Routing agrees with the sorted positions.
        assert_eq!(plan.segment_index(&SegmentId::from("seg-a")), Some(0));
        assert_eq!(plan.segment_index(&SegmentId::from("seg-c")), Some(2));
    }
}
