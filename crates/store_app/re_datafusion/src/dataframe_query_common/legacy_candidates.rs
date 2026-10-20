use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Array, RecordBatch};
use arrow::datatypes::{FieldRef, Schema};
use arrow::record_batch::RecordBatchOptions;
use datafusion::common::{DataFusionError, exec_datafusion_err};
use re_dataframe::TimelineName;
use re_dataframe::external::re_chunk::TimeColumn;
use re_protos::cloud::v1alpha1::ext::{
    ChunkCandidatesDataframe, ChunkMode, FillMode, FindChunksRequest, LegacyPlan,
    QueryDatasetRequest, RangeSelection, adapt_legacy,
};

/// Converts a client-planned [`QueryDatasetRequest`] into the wire [`FindChunksRequest`], through
/// the legacy adapter the servers share for their compatibility path.
///
/// `index` is the reader's timeline (`QueryExpression::filtered_index`). It gives a
/// [`LegacyPlan::FullScan`] plan, which carries no timeline, a selection timeline; a
/// [`LegacyPlan::Find`] plan keeps its own, even when it differs from `index`.
///
/// `synthesize_latest_at` is the reader's fill setting. It decides whether a
/// [`ChunkMode::Samples`] plan also selects the latest-at chunks.
///
/// `adapt_legacy` rejects a shape only when the planner would never build it, so a rejection here
/// maps to [`DataFusionError::Internal`].
///
/// TODO(RR-5817): temporary. Once the SDK no longer falls back to `QueryDataset`, the planner
/// builds a [`FindChunksRequest`] directly, and this function goes with `adapt_legacy`.
pub fn to_wire(
    request: &QueryDatasetRequest,
    index: Option<TimelineName>,
    synthesize_latest_at: bool,
) -> Result<FindChunksRequest, DataFusionError> {
    let plan = adapt_legacy(request.clone())
        .map_err(|err| DataFusionError::Internal(err.message().to_owned()))?;

    Ok(match plan {
        LegacyPlan::Find(mut request) => {
            if let ChunkMode::Samples { fill_mode, .. } = &mut request.mode {
                *fill_mode = if synthesize_latest_at {
                    FillMode::LatestAt
                } else {
                    FillMode::None
                };
            }
            request
        }

        LegacyPlan::FullScan {
            segments,
            contents,
            direct_url_mode,
        } => {
            let mode = match index {
                Some(timeline) => ChunkMode::Range {
                    segments,
                    range: RangeSelection {
                        timeline,
                        start_inclusive: None,
                        end_inclusive: None,
                        fill_mode: FillMode::None,
                    },
                },
                None => ChunkMode::Static { segments },
            };

            FindChunksRequest {
                contents,
                mode,
                direct_url_mode,
            }
        }
    })
}

/// Converts legacy `query_dataset` response batches into the fixed [`ChunkCandidatesDataframe`]
/// schema, alongside whether every batch's `{index}:start` data was complete enough to trust for
/// emission ordering.
///
/// Completeness is decided across every *populated* batch before any of them is converted: if any
/// non-empty batch lacks the `{index}:start` column, every output batch gets an all-null
/// `chunk_timeline_start` and the returned flag is `false`. An absent column means the response
/// supplies no start bounds on the reader's timeline. A null value in a present column means
/// the chunk has no time on that index. An all-null output column does not tell the two apart,
/// hence the flag.
///
/// With `index = None` (a static reader has no selection timeline to be incomplete about) or for
/// an empty batch that lacks the column, the batch gets an all-null `chunk_timeline_start` without
/// affecting the completeness flag.
///
/// TODO(RR-5817): temporary. Only the SDK's `QueryDataset` fallback produces legacy batches.
pub fn normalize_legacy_candidate_batches(
    batches: Vec<RecordBatch>,
    index: Option<TimelineName>,
) -> Result<(Vec<RecordBatch>, bool), DataFusionError> {
    let start_col_name = index.map(|timeline| format!("{timeline}:start"));

    let complete = match &start_col_name {
        Some(name) => batches
            .iter()
            .filter(|batch| batch.num_rows() > 0)
            .all(|batch| batch.column_by_name(name).is_some()),
        None => true,
    };

    let source_col_name = start_col_name.as_deref().filter(|_| complete);
    let batches = batches
        .into_iter()
        .map(|batch| convert_legacy_candidate_batch(&batch, source_col_name))
        .collect::<Result<Vec<_>, _>>()?;

    Ok((batches, complete))
}

/// Rebuilds one legacy `query_dataset` batch as a [`ChunkCandidatesDataframe`] batch: the ten
/// columns both schemas share, reusing their Arrow arrays unchanged, plus `chunk_timeline_start`
/// derived from `source_col_name` per [`build_chunk_timeline_start`].
fn convert_legacy_candidate_batch(
    batch: &RecordBatch,
    source_col_name: Option<&str>,
) -> Result<RecordBatch, DataFusionError> {
    let target_schema = ChunkCandidatesDataframe::min_schema();

    let mut fields: Vec<FieldRef> = Vec::with_capacity(target_schema.fields().len());
    let mut columns: Vec<ArrayRef> = Vec::with_capacity(target_schema.fields().len());

    for field in target_schema.fields() {
        if field.name() == ChunkCandidatesDataframe::COLUMN_CHUNK_TIMELINE_START_NAME {
            let (start_field, start_array) = build_chunk_timeline_start(batch, source_col_name)?;
            fields.push(start_field);
            columns.push(start_array);
        } else {
            let array = batch.column_by_name(field.name()).ok_or_else(|| {
                exec_datafusion_err!("candidate batch is missing column `{}`", field.name())
            })?;
            fields.push(Arc::clone(field));
            columns.push(Arc::clone(array));
        }
    }

    RecordBatch::try_new_with_options(
        Arc::new(Schema::new_with_metadata(fields, HashMap::default())),
        columns,
        &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
    )
    .map_err(|err| exec_datafusion_err!("building candidate batch: {err}"))
}

/// Builds the `chunk_timeline_start` column for one batch: the `source_col_name` array read
/// through [`TimeColumn::read_nullable_array`], or an all-null column when there is no source
/// column name (no reader index, or an incomplete response) or this (necessarily empty) batch
/// lacks the column.
///
/// TODO(RR-5817): temporary. Only the SDK's `QueryDataset` fallback produces legacy batches.
fn build_chunk_timeline_start(
    batch: &RecordBatch,
    source_col_name: Option<&str>,
) -> Result<(FieldRef, ArrayRef), DataFusionError> {
    let field = ChunkCandidatesDataframe::COLUMN_CHUNK_TIMELINE_START.arrow_field_ref();

    let Some((col_name, start_col)) =
        source_col_name.and_then(|col_name| Some((col_name, batch.column_by_name(col_name)?)))
    else {
        let all_null = Arc::new(Int64Array::from(vec![None; batch.num_rows()])) as ArrayRef;
        return Ok((field, all_null));
    };

    let (values, nulls) = TimeColumn::read_nullable_array(start_col.as_ref())
        .map_err(|err| exec_datafusion_err!("`{col_name}` column has unsupported type: {err}"))?;

    Ok((field, Arc::new(Int64Array::new(values, nulls))))
}

#[cfg(test)]
mod tests {
    use arrow::array::Array as _;
    use arrow::datatypes::Field;
    use re_protos::cloud::v1alpha1::ext::{Query, QueryLatestAt, QueryRange, SegmentSelection};
    use re_protos::common::v1alpha1::ext::SegmentId;

    use super::*;

    // -------------------------------------------------------------------
    // `to_wire`
    // -------------------------------------------------------------------

    #[test]
    fn to_wire_samples_follow_the_reader_fill_setting() {
        use re_log_types::TimeInt;

        let request = QueryDatasetRequest {
            segment_ids: vec![SegmentId::from("seg1")],
            query: Some(Query {
                latest_at: Some(QueryLatestAt {
                    index: Some(TimelineName::from("t")),
                    at: TimeInt::STATIC,
                    per_segment_values: vec![vec![5]],
                }),
                ..Query::default()
            }),
            ..Default::default()
        };

        for (synthesize_latest_at, expected_fill_mode) in
            [(false, FillMode::None), (true, FillMode::LatestAt)]
        {
            let wire = to_wire(
                &request,
                Some(TimelineName::from("t")),
                synthesize_latest_at,
            )
            .expect("the planner never builds a rejected shape");
            let ChunkMode::Samples { fill_mode, .. } = wire.mode else {
                panic!(
                    "sample times must become a Samples request, got {:?}",
                    wire.mode
                );
            };
            assert_eq!(
                fill_mode, expected_fill_mode,
                "synthesize_latest_at={synthesize_latest_at}"
            );
        }
    }

    #[test]
    fn to_wire_full_scan_maps_index_to_selection_timeline() {
        for segment_ids in [vec![], vec![SegmentId::from("seg1")]] {
            let expected_segments = if segment_ids.is_empty() {
                SegmentSelection::All
            } else {
                SegmentSelection::Explicit(segment_ids.iter().cloned().collect())
            };

            // Neither `latest_at` nor `range`: `adapt_legacy` reports `FullScan`, which carries
            // no timeline of its own.
            let request = QueryDatasetRequest {
                segment_ids: segment_ids.clone(),
                query: Some(Query::default()),
                ..Default::default()
            };

            for timeline_name in ["t", "u"] {
                let timeline = TimelineName::from(timeline_name);
                let wire =
                    to_wire(&request, Some(timeline), false).expect("FullScan is never rejected");
                assert_eq!(
                    wire.mode,
                    ChunkMode::Range {
                        segments: expected_segments.clone(),
                        range: RangeSelection {
                            timeline,
                            start_inclusive: None,
                            end_inclusive: None,
                            fill_mode: FillMode::None,
                        },
                    },
                    "reader index {timeline_name} must become an unbounded Range on that timeline"
                );
            }

            let wire = to_wire(&request, None, false).expect("FullScan is never rejected");
            assert_eq!(
                wire.mode,
                ChunkMode::Static {
                    segments: expected_segments,
                },
                "a static reader (no index) must turn FullScan into Static"
            );
        }
    }

    #[test]
    fn to_wire_find_case_preserves_adapter_output() {
        use re_log_types::{AbsoluteTimeRange, TimeInt};

        let timeline = TimelineName::from("t");
        let request = QueryDatasetRequest {
            entity_paths: vec![re_log_types::EntityPath::from("/a")],
            select_all_entity_paths: false,
            generate_direct_urls: false,
            query: Some(Query {
                range: Some(QueryRange {
                    index: timeline,
                    index_range: AbsoluteTimeRange::new(
                        TimeInt::new_temporal(0),
                        TimeInt::new_temporal(10),
                    ),
                }),
                ..Query::default()
            }),
            ..Default::default()
        };

        let expected = match adapt_legacy(request.clone()).expect("valid shape") {
            LegacyPlan::Find(expected) => expected,
            LegacyPlan::FullScan { .. } => panic!("expected a `Find` plan"),
        };

        let wire = to_wire(&request, Some(timeline), false).expect("valid shape");
        assert_eq!(
            wire, expected,
            "to_wire must not alter the adapter's own output"
        );
        assert_eq!(
            wire.direct_url_mode,
            re_protos::cloud::v1alpha1::ext::DirectUrlMode::Omit
        );
    }

    #[test]
    fn to_wire_maps_legacy_direct_url_flags_for_find_and_full_scan() {
        use re_protos::cloud::v1alpha1::ext::DirectUrlMode;

        for (generate_direct_urls, unsigned_direct_urls, expected) in [
            (false, false, DirectUrlMode::Omit),
            (false, true, DirectUrlMode::Omit),
            (true, false, DirectUrlMode::Signed),
            (true, true, DirectUrlMode::Unsigned),
        ] {
            for query in [
                Query::default(),
                Query {
                    latest_at: Some(QueryLatestAt::new_static()),
                    ..Query::default()
                },
            ] {
                let request = QueryDatasetRequest {
                    generate_direct_urls,
                    unsigned_direct_urls,
                    query: Some(query),
                    ..Default::default()
                };
                let wire = to_wire(&request, None, false).expect("valid legacy request");
                assert_eq!(wire.direct_url_mode, expected);
            }
        }
    }

    #[test]
    fn to_wire_rejected_shape_maps_to_internal_error() {
        let request = QueryDatasetRequest {
            chunk_ids: vec![re_dataframe::external::re_chunk::ChunkId::new()],
            ..Default::default()
        };

        let err =
            to_wire(&request, None, false).expect_err("`chunk_ids` has no `FindChunks` equivalent");
        assert!(
            matches!(err, DataFusionError::Internal(_)),
            "unexpected error: {err}"
        );
    }

    // -------------------------------------------------------------------
    // `normalize_legacy_candidate_batches`
    // -------------------------------------------------------------------

    /// A legacy `query_dataset` batch with `num_rows` rows and no `{index}:start` column, ready
    /// for [`with_extra_column`] to attach one.
    fn legacy_batch(num_rows: usize) -> RecordBatch {
        use re_dataframe::external::re_chunk::ChunkId;
        use re_log_types::EntityPath;
        use re_types_core::LayerName;

        let segment_id = SegmentId::from("seg1");
        let entity_path = EntityPath::from("/e");
        type Df = ChunkCandidatesDataframe;
        let candidates = Df {
            chunk_id: Df::COLUMN_CHUNK_ID.new_from_values((0..num_rows).map(|_| ChunkId::new())),
            chunk_segment_id: Df::COLUMN_CHUNK_SEGMENT_ID
                .new_from_values(vec![segment_id; num_rows]),
            rerun_segment_layer: Df::COLUMN_RERUN_SEGMENT_LAYER
                .new_from_values(vec![LayerName::from("base"); num_rows]),
            chunk_key: Df::COLUMN_CHUNK_KEY.new_from_values(vec![&[] as &[u8]; num_rows]),
            chunk_entity_path: Df::COLUMN_CHUNK_ENTITY_PATH
                .new_from_values(vec![entity_path; num_rows]),
            chunk_is_static: Df::COLUMN_CHUNK_IS_STATIC.new_from_values(vec![false; num_rows]),
            chunk_byte_len: Df::COLUMN_CHUNK_BYTE_LEN.new_from_values(vec![10_u64; num_rows]),
            chunk_byte_size_uncompressed: Df::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED
                .new_from_values(vec![Some(10_u64); num_rows]),
            chunk_timeline_start: Df::COLUMN_CHUNK_TIMELINE_START
                .new_from_values(vec![None::<i64>; num_rows]),
            rerun_layer_direct_url: Df::COLUMN_RERUN_LAYER_DIRECT_URL
                .try_new_from_values(vec![None::<String>; num_rows])
                .expect("valid direct-url column"),
            rerun_layer_direct_url_expires_at: Df::COLUMN_RERUN_LAYER_DIRECT_URL_EXPIRES_AT
                .try_new_from_values(vec![None::<i64>; num_rows])
                .expect("valid expiration column"),
        }
        .into_record_batch()
        .expect("valid schema");

        re_protos::cloud::v1alpha1::ext::encode_legacy_candidates(candidates, None)
            .expect("a candidate batch always encodes")
    }

    /// `batch` with an extra column appended (a `{timeline}:start` column, or any other extra
    /// column an old Hub response might carry alongside it).
    fn with_extra_column(batch: &RecordBatch, name: &str, array: ArrayRef) -> RecordBatch {
        let mut fields: Vec<FieldRef> = batch.schema().fields().iter().cloned().collect();
        let mut columns: Vec<ArrayRef> = batch.columns().to_vec();
        fields.push(Arc::new(Field::new(name, array.data_type().clone(), true)));
        columns.push(array);
        RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(fields, HashMap::default())),
            columns,
            &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
        )
        .expect("adding a column keeps the batch valid")
    }

    /// The `chunk_timeline_start` column's values, in row order.
    fn starts_of(batch: &RecordBatch) -> Vec<Option<i64>> {
        let col = batch
            .column_by_name(ChunkCandidatesDataframe::COLUMN_CHUNK_TIMELINE_START_NAME)
            .expect("normalized batches always carry `chunk_timeline_start`")
            .as_any()
            .downcast_ref::<Int64Array>()
            .expect("`chunk_timeline_start` is always Int64");
        (0..col.len())
            .map(|i| (!col.is_null(i)).then(|| col.value(i)))
            .collect()
    }

    #[test]
    fn normalize_int64_start_column() {
        let batch = with_extra_column(
            &legacy_batch(2),
            "t:start",
            Arc::new(Int64Array::from(vec![Some(10), Some(20)])),
        );

        let (out, complete) =
            normalize_legacy_candidate_batches(vec![batch], Some(TimelineName::from("t")))
                .expect("valid batch");

        assert!(complete);
        assert_eq!(out.len(), 1);
        assert_eq!(starts_of(&out[0]), vec![Some(10), Some(20)]);
    }

    #[test]
    fn normalize_old_hub_shaped_batch_timestamp_ns_start_and_end() {
        use arrow::array::TimestampNanosecondArray;

        let batch = with_extra_column(
            &legacy_batch(2),
            "t:start",
            Arc::new(TimestampNanosecondArray::from(vec![
                Some(100_i64),
                Some(200_i64),
            ])),
        );
        // Old Hub responses also carry a `:end` column alongside `:start`; it must be dropped,
        // not mistaken for the selection-timeline start.
        let batch = with_extra_column(
            &batch,
            "t:end",
            Arc::new(TimestampNanosecondArray::from(vec![
                Some(150_i64),
                Some(250_i64),
            ])),
        );

        let (out, complete) =
            normalize_legacy_candidate_batches(vec![batch], Some(TimelineName::from("t")))
                .expect("valid batch");

        assert!(complete);
        assert!(
            out[0].column_by_name("t:end").is_none(),
            "only the ten base columns and `chunk_timeline_start` survive"
        );
        assert_eq!(starts_of(&out[0]), vec![Some(100), Some(200)]);
    }

    #[test]
    fn normalize_batch_with_no_start_column_is_incomplete() {
        let batch = legacy_batch(2);

        let (out, complete) =
            normalize_legacy_candidate_batches(vec![batch], Some(TimelineName::from("t")))
                .expect("a missing `:start` column is a fallback, not an error");

        assert!(!complete);
        assert_eq!(starts_of(&out[0]), vec![None, None]);
    }

    #[test]
    fn normalize_time64_and_duration_ns_start_columns() {
        use arrow::array::{DurationNanosecondArray, Time64NanosecondArray};

        let start_columns: [ArrayRef; 2] = [
            Arc::new(Time64NanosecondArray::from(vec![Some(30_i64), None])),
            Arc::new(DurationNanosecondArray::from(vec![Some(30_i64), None])),
        ];
        for start_column in start_columns {
            let data_type = start_column.data_type().clone();
            let batch = with_extra_column(&legacy_batch(2), "t:start", start_column);

            let (out, complete) =
                normalize_legacy_candidate_batches(vec![batch], Some(TimelineName::from("t")))
                    .expect("valid batch");

            assert!(complete, "{data_type}");
            assert_eq!(starts_of(&out[0]), vec![Some(30), None], "{data_type}");
        }
    }

    #[test]
    fn normalize_unsupported_start_dtype_is_an_error() {
        use arrow::array::Float64Array;

        let batch = with_extra_column(
            &legacy_batch(2),
            "t:start",
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
        );

        let err = normalize_legacy_candidate_batches(vec![batch], Some(TimelineName::from("t")))
            .expect_err("an unsupported `:start` dtype must not be silently accepted");
        assert!(
            matches!(err, DataFusionError::Execution(_)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn normalize_populated_missing_column_batch_clears_every_batch() {
        let with_start = with_extra_column(
            &legacy_batch(1),
            "t:start",
            Arc::new(Int64Array::from(vec![Some(10)])),
        );
        let without_start = legacy_batch(1); // populated, no `:start` column

        let (out, complete) = normalize_legacy_candidate_batches(
            vec![with_start, without_start],
            Some(TimelineName::from("t")),
        )
        .expect("valid batches");

        assert!(
            !complete,
            "a populated batch missing `:start` must force full fallback"
        );
        for batch in &out {
            assert_eq!(
                starts_of(batch),
                vec![None],
                "every batch's starts must be cleared, including the one that had them"
            );
        }
    }

    #[test]
    fn normalize_empty_missing_column_batch_does_not_discard_known_starts() {
        let with_start = with_extra_column(
            &legacy_batch(1),
            "t:start",
            Arc::new(Int64Array::from(vec![Some(10)])),
        );
        let empty_without_start = legacy_batch(0); // no rows, no `:start` column

        let (out, complete) = normalize_legacy_candidate_batches(
            vec![with_start, empty_without_start],
            Some(TimelineName::from("t")),
        )
        .expect("valid batches");

        assert!(
            complete,
            "an empty batch missing `:start` carries no information to lose"
        );
        assert_eq!(starts_of(&out[0]), vec![Some(10)]);
        assert!(starts_of(&out[1]).is_empty());
    }

    #[test]
    fn normalize_static_reader_yields_all_null_and_complete() {
        let (out, complete) =
            normalize_legacy_candidate_batches(vec![legacy_batch(2)], None).expect("valid batch");

        assert!(
            complete,
            "a static reader has no selection timeline to be incomplete about"
        );
        assert_eq!(starts_of(&out[0]), vec![None, None]);
    }
}
