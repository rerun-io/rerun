//! Helpers shared by the `SegmentStreamExec` implementations (native and wasm)
//! and by `LocalChunkStoreTableProvider`: the plan claims a segment stream
//! makes, the schema shaping those claims depend on, the partition routing they
//! promise, and the chunk-info aggregates that seed the analytics span.

use ahash::{HashMap, HashMapExt as _, HashSet};
use arrow::array::{ArrayRef, RecordBatch};
use arrow::compute::SortOptions;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::common::{DataFusionError, exec_datafusion_err};
use datafusion::physical_expr::expressions::Column;
use datafusion::physical_expr::{EquivalenceProperties, Partitioning, PhysicalSortExpr};
use datafusion::physical_plan::PlanProperties;
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use itertools::Itertools as _;
use re_dataframe::Index;
use re_protos::cloud::v1alpha1::ext::{QueryDatasetDataframe, ScanSegmentTableDataframe};
use re_protos::common::v1alpha1::ext::SegmentId;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Look up a column by name in `schema` and build a physical [`Column`]
/// expression with the correct index.
///
/// Physical columns resolve by *index* at execution time, so a hardcoded index
/// goes silently wrong as soon as a projection moves the column. Returns `None`
/// when `name` is absent or ambiguous (Arrow allows repeated field names): a
/// claim on the wrong column is wrong output, not a slowdown, so ambiguity must
/// degrade to claiming nothing.
pub(crate) fn physical_column_by_name(schema: &Schema, name: &str) -> Option<Column> {
    let mut matches = schema
        .fields()
        .iter()
        .positions(|field| field.name() == name);

    let idx = matches.next()?;

    if matches.next().is_some() {
        re_log::debug_warn_once!(
            "Refusing to resolve ambiguous column {name:?}: it appears more than once in the schema, so no ordering or partitioning can be claimed on it"
        );
        return None;
    }

    Some(Column::new(name, idx))
}

/// Build the [`PlanProperties`] (output-ordering and hash-partitioning claims)
/// for a `SegmentStreamExec` over the given projected schema.
///
/// The stream emits rows ordered by `[rerun_segment_id ASC, <sort_index> ASC]`
/// per partition, with segments hash-distributed across partitions by
/// `rerun_segment_id`. Both claims are conditional on `rerun_segment_id`
/// surviving the projection unambiguously: a violated claim is silently wrong
/// output for downstream sorted-mode operators, not a slowdown, so when the
/// column is projected away — or is ambiguous, see
/// [`physical_column_by_name`] — we claim nothing. An unresolvable
/// `sort_index` only weakens the ordering to segment-only, which stays true.
///
/// The claim only elides a sort for `NULLS FIRST` consumers (e.g.
/// datafusion-python's `Expr.sort` default). DataFusion demands an exact
/// `nulls_first` match on *nullable* sort expressions, and `re_sorbet` declares
/// every index column nullable, so SQL's bare `ORDER BY <index>` — which is
/// `NULLS LAST` — still pays a full materializing sort.
///
/// TODO(tsaucer): declare the sort-index field non-nullable so the claim
/// satisfies both nulls placements. The field is truthfully never null and the
/// change works (it elides the sort on the in-process path), but it also makes
/// DataFusion's window-expression reversal reachable, which is currently
/// broken. Blocked on both of:
///   - <https://github.com/apache/datafusion/issues/24884>
///     `EnforceSorting` reverses a window expression and renames its output
///     field without rewriting the parent projection, so any
///     `first_value`/`last_value` window pair (one ASC, one DESC) over a
///     non-nullable index fails to plan. Reproduced with a plain parquet
///     source and no Rerun code.
///   - <https://github.com/apache/datafusion/issues/24885>
///     the same query is fine when written against the window-UDF variants,
///     which datafusion-python does not expose — so users cannot work around
///     the first issue from Python.
///
/// Note also that the claim is *not* currently realized through the
/// datafusion-ffi path at all (`ORDER BY rerun_segment_id, <index>` still plans
/// a `SortExec` from Python), so re-landing this only pays off in-process until
/// that is understood separately.
pub(crate) fn segment_stream_plan_properties(
    projected_schema: &SchemaRef,
    sort_index: Option<Index>,
    num_partitions: usize,
) -> PlanProperties {
    let Some(segment_id_col) = physical_column_by_name(
        projected_schema,
        ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
    ) else {
        return PlanProperties::new(
            EquivalenceProperties::new(Arc::clone(projected_schema)),
            Partitioning::UnknownPartitioning(num_partitions),
            EmissionType::Incremental,
            Boundedness::Bounded,
        );
    };

    let sort_index_col =
        sort_index.and_then(|index| physical_column_by_name(projected_schema, index.as_str()));
    let ordering =
        std::iter::chain(std::iter::once(segment_id_col.clone()), sort_index_col).map(|col| {
            PhysicalSortExpr::new(
                Arc::new(col),
                SortOptions {
                    descending: false,
                    nulls_first: true,
                },
            )
        });

    PlanProperties::new(
        EquivalenceProperties::new_with_orderings(Arc::clone(projected_schema), [ordering]),
        Partitioning::Hash(vec![Arc::new(segment_id_col)], num_partitions),
        EmissionType::Incremental,
        Boundedness::Bounded,
    )
}

pub(crate) fn prepend_string_column_schema(schema: &Schema, column_name: &str) -> Schema {
    let mut fields = vec![Field::new(column_name, DataType::Utf8, false)];
    fields.extend(schema.fields().iter().map(|f| (**f).clone()));
    Schema::new_with_metadata(fields, schema.metadata.clone())
}

/// Takes each field's data type from the corresponding array in `columns`, keeping the
/// field name, nullability and metadata from `schema`.
///
/// A `QueryHandle`'s schema is built from the `ChunkStore`'s
/// `ComponentColumnDescriptor`s, and `ChunkStore` rewrites the outer list field of every
/// component column to `Field::new("item", value_type, true)` — discarding the name and
/// nullability the producer actually used (see `re_chunk_store::store_schema`). The row
/// data, on the other hand, is sliced straight out of the chunks, so it keeps the
/// original field. For a producer that emits a non-nullable outer list field the two
/// disagree, and `RecordBatch::try_new` rejects the batch: arrow's `equals_datatype`
/// compares child-field nullability. See <https://github.com/rerun-io/rerun/issues/12887>.
///
/// Building the intermediate batch against the *actual* data types side-steps that
/// mismatch; the following `align_record_batch_to_schema` then widens the columns to the
/// DataFusion output schema, which is what resolves the nullability difference for real.
pub(crate) fn schema_with_array_datatypes(schema: &Schema, columns: &[ArrayRef]) -> Schema {
    re_log::debug_assert_eq!(schema.fields().len(), columns.len());

    let fields = std::iter::zip(schema.fields(), columns)
        .map(|(field, column)| {
            if field.data_type() == column.data_type() {
                (**field).clone()
            } else {
                Field::clone(field).with_data_type(column.data_type().clone())
            }
        })
        .collect::<Vec<_>>();

    Schema::new_with_metadata(fields, schema.metadata.clone())
}

/// Hash a segment id for DataFusion partition routing.
///
/// Hashes the underlying string with DataFusion's `HashValue` so the result
/// matches `RepartitionExec`'s hashing of the segment-id string column.
pub(crate) fn segment_partition_hash(segment_id: &SegmentId) -> u64 {
    use datafusion::common::hash_utils::HashValue as _;
    use datafusion::physical_plan::repartition::REPARTITION_RANDOM_STATE;

    segment_id
        .as_str()
        .hash_one(REPARTITION_RANDOM_STATE.random_state())
}

/// Whether `RepartitionExec`'s hashing routes `segment_id` to `partition`.
///
/// The modulo must be taken on the full u64 hash: narrowing to `usize` first
/// truncates to 32 bits on wasm32, which disagrees with `RepartitionExec`'s
/// `hash % num_partitions` whenever `num_partitions` is not a power of two —
/// and a divergence here silently breaks the `Partitioning::Hash` claim made
/// by [`segment_stream_plan_properties`].
pub(crate) fn segment_belongs_to_partition(
    segment_id: &SegmentId,
    partition: usize,
    num_partitions: usize,
) -> bool {
    segment_partition_hash(segment_id) % num_partitions as u64 == partition as u64
}

/// We need to create `num_partitions` of DataFusion partition stream outputs, each of
/// which will be fed from multiple `rerun_segment_id` sources. The partitioning
/// output is a hash of the `rerun_segment_id`. We will reuse some of the
/// underlying execution code from `DataFusion`'s `RepartitionExec` to compute
/// these DataFusion partition IDs, just to be certain they match partitioning generated
/// from sources other than Rerun gRPC services.
/// This function will do the relevant grouping of chunk infos by chunk's segment id,
/// and we will eventually fire individual queries for each group. Segments must be ordered,
/// see `SegmentStreamExec::try_new` for more details.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn group_chunk_infos_by_segment_id(
    chunk_info_batches: &[RecordBatch],
) -> Result<Arc<BTreeMap<SegmentId, Vec<RecordBatch>>>, DataFusionError> {
    let mut results: BTreeMap<SegmentId, Vec<RecordBatch>> = BTreeMap::new();

    for batch in chunk_info_batches {
        let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(batch)
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        // group rows by segment ID
        let mut segment_rows: BTreeMap<SegmentId, Vec<usize>> = BTreeMap::new();
        for (row_idx, segment_id) in segment_ids.into_iter_owned().enumerate() {
            segment_rows.entry(segment_id).or_default().push(row_idx);
        }

        for (segment_id, row_indices) in segment_rows {
            if row_indices.is_empty() {
                continue;
            }

            let segment_batch = re_arrow_util::take_record_batch(batch, &row_indices)?;

            results.entry(segment_id).or_default().push(segment_batch);
        }
    }

    Ok(Arc::new(results))
}

/// Compact, display-friendly snapshot of the plan-time decisions that drove a scan.
///
/// Surfaced via `DisplayAs::Verbose` on `SegmentStreamExec` so plain `EXPLAIN`
/// (without `ANALYZE`) shows the most useful planning-phase decisions.
#[derive(Debug, Clone)]
pub(crate) struct PlanSummary {
    pub query_type: &'static str,
    pub query_chunks: usize,
    pub query_segments: usize,
    pub query_bytes: u64,
    pub filters_pushed_down: usize,
    pub filters_applied_client_side: usize,
    pub entity_path_narrowing_applied: bool,
}

impl PlanSummary {
    pub fn from_query_info(info: &crate::analytics::QueryInfo) -> Self {
        Self {
            query_type: info.query_type.as_str(),
            query_chunks: info.query_chunks,
            query_segments: info.query_segments,
            query_bytes: info.query_bytes,
            filters_pushed_down: info.filters_pushed_down,
            filters_applied_client_side: info.filters_applied_client_side,
            entity_path_narrowing_applied: info.entity_path_narrowing_applied,
        }
    }
}

/// Aggregates derived from the deduplicated chunk metadata returned by `query_dataset`.
///
/// These are cheap zero-copy Arrow reads (no per-element allocation except the
/// segment histogram map). The scan path computes them once to seed the
/// analytics span without adding an extra pass.
#[derive(Default)]
pub(crate) struct ChunkInfoAggregates {
    pub chunks: usize,
    pub segments: usize,
    pub layers: usize,
    pub bytes: u64,
    pub chunks_per_segment_min: u32,
    pub chunks_per_segment_max: u32,
    pub chunks_per_segment_mean: f32,
}

pub(crate) fn compute_chunk_info_aggregates(batch: &RecordBatch) -> ChunkInfoAggregates {
    let chunks = batch.num_rows();

    // Lenient: these are analytics aggregates — a missing or mistyped column yields zeros.
    let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
        .extract(batch)
        .ok();
    let layer_names = QueryDatasetDataframe::COLUMN_RERUN_SEGMENT_LAYER
        .extract(batch)
        .ok();
    let byte_lens = QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN
        .extract(batch)
        .ok();

    // Segment count + per-segment histogram in one pass
    let mut per_segment: HashMap<&str, u32> = HashMap::new();
    for v in segment_ids.iter().flatten() {
        *per_segment.entry(v).or_default() += 1;
    }
    let segments = per_segment.len();
    let (chunks_per_segment_min, chunks_per_segment_max) = per_segment
        .into_values()
        .fold((u32::MAX, 0u32), |(min, max), v| (min.min(v), max.max(v)));
    // Clamp the sentinel back to 0 when the histogram was empty.
    let chunks_per_segment_min = if segments == 0 {
        0
    } else {
        chunks_per_segment_min
    };
    let chunks_per_segment_mean = if segments == 0 {
        0.0
    } else {
        // chunks fits in u32 for realistic queries; precision loss is acceptable for analytics.
        chunks as f32 / segments as f32
    };

    let layers = layer_names.map_or(0, |col| col.iter().collect::<HashSet<_>>().len());

    let bytes: u64 = byte_lens.map_or(0, |col| col.iter().sum());

    ChunkInfoAggregates {
        chunks,
        segments,
        layers,
        bytes,
        chunks_per_segment_min,
        chunks_per_segment_max,
        chunks_per_segment_mean,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow::array::{FixedSizeBinaryBuilder, StringArray};
    use arrow::record_batch::RecordBatchOptions;
    use re_protos::cloud::v1alpha1::ext;

    use super::*;

    fn plan_props_schema(fields: &[&str]) -> SchemaRef {
        Arc::new(Schema::new_with_metadata(
            fields
                .iter()
                .map(|name| Field::new(*name, DataType::Int64, true))
                .collect::<Vec<_>>(),
            Default::default(),
        ))
    }

    /// Ordering columns as `"name@index"` strings (the `Display` of a
    /// physical `Column`), so tests can assert on the resolved index.
    fn ordering_columns(props: &datafusion::physical_plan::PlanProperties) -> Vec<String> {
        props
            .eq_properties
            .oeq_class()
            .iter()
            .next()
            .map(|ordering| {
                ordering
                    .iter()
                    .map(|sort_expr| sort_expr.expr.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn segment_stream_claims_resolve_column_indices_by_name() {
        // segment id is NOT column 0 here: a projection has moved it.
        let schema = plan_props_schema(&[
            "log_time",
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            "data",
        ]);
        let props = segment_stream_plan_properties(&schema, Some(Index::from("log_time")), 4);

        assert_eq!(
            ordering_columns(&props),
            vec![
                format!(
                    "{}@1",
                    ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME
                ),
                "log_time@0".to_owned(),
            ]
        );

        let datafusion::physical_expr::Partitioning::Hash(exprs, n) = props.partitioning else {
            panic!("expected hash partitioning, got {:?}", props.partitioning);
        };
        assert_eq!(n, 4);
        assert_eq!(
            exprs[0].to_string(),
            format!(
                "{}@1",
                ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME
            )
        );
    }

    /// Pins the asymmetry the sort-index field's nullability forces on us:
    /// `re_sorbet` declares index columns nullable, DataFusion demands an exact
    /// `nulls_first` match on nullable sort expressions, so the claim satisfies
    /// `NULLS FIRST` (datafusion-python's `Expr.sort` default) but not
    /// `NULLS LAST` (SQL's bare `ASC`), which still pays a full sort.
    ///
    /// Declaring the field non-nullable fixes this and is truthful, but is
    /// blocked on the DataFusion bugs listed on
    /// [`segment_stream_plan_properties`]. If this test starts failing on the
    /// `NULLS LAST` case, that work has landed — update the doc comment there
    /// and drop the `expected_satisfied` distinction.
    #[test]
    fn segment_stream_ordering_satisfies_only_nulls_first_on_a_nullable_index() {
        use arrow::compute::SortOptions;
        use datafusion::physical_expr::{PhysicalSortExpr, expressions::Column};

        let segment_id = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME;
        // Mirror the real provider schema construction: non-nullable segment
        // id prepended to a server schema whose index column is nullable.
        let server_schema = Schema::new_with_metadata(
            vec![
                Field::new("log_time", DataType::Int64, true),
                Field::new("data", DataType::Int64, true),
            ],
            Default::default(),
        );
        let schema: SchemaRef = Arc::new(prepend_string_column_schema(&server_schema, segment_id));

        let props = segment_stream_plan_properties(&schema, Some(Index::from("log_time")), 4);

        for (nulls_first, expected_satisfied) in [(true, true), (false, false)] {
            let required = vec![
                PhysicalSortExpr::new(
                    Arc::new(Column::new(segment_id, 0)),
                    SortOptions::new(false, nulls_first),
                ),
                PhysicalSortExpr::new(
                    Arc::new(Column::new("log_time", 1)),
                    SortOptions::new(false, nulls_first),
                ),
            ];
            assert_eq!(
                props.eq_properties.ordering_satisfy(required).unwrap(),
                expected_satisfied,
                "ORDER BY {segment_id}, log_time with nulls_first={nulls_first}"
            );
        }
    }

    #[test]
    fn segment_stream_claims_nothing_without_segment_id_column() {
        // segment id projected away: claiming ordering or hash partitioning
        // would be silently wrong.
        let schema = plan_props_schema(&["log_time", "data"]);
        let props = segment_stream_plan_properties(&schema, Some(Index::from("log_time")), 4);

        assert_eq!(ordering_columns(&props), Vec::<String>::new());
        assert!(matches!(
            props.partitioning,
            datafusion::physical_expr::Partitioning::UnknownPartitioning(4)
        ));
    }

    #[test]
    fn segment_stream_claims_segment_only_ordering_without_sort_index() {
        let schema = plan_props_schema(&[
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            "data",
        ]);
        let props = segment_stream_plan_properties(&schema, None, 2);

        assert_eq!(
            ordering_columns(&props),
            vec![format!(
                "{}@0",
                ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME
            )]
        );
    }

    #[test]
    fn segment_stream_claims_nothing_when_segment_id_is_ambiguous() {
        // Arrow permits repeated field names. Whichever index we picked would
        // be a coin flip, and a claim on the wrong column is silently wrong
        // output, so both claims must be dropped.
        let segment_id = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME;
        let schema = plan_props_schema(&[segment_id, "log_time", segment_id]);
        let props = segment_stream_plan_properties(&schema, Some(Index::from("log_time")), 4);

        assert_eq!(ordering_columns(&props), Vec::<String>::new());
        assert!(matches!(
            props.partitioning,
            datafusion::physical_expr::Partitioning::UnknownPartitioning(4)
        ));
    }

    #[test]
    fn segment_stream_drops_only_the_sort_index_when_it_is_ambiguous() {
        // An unresolvable sort index weakens the ordering to segment-only,
        // which is still true, so the segment claims survive.
        let segment_id = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME;
        let schema = plan_props_schema(&[segment_id, "log_time", "log_time"]);
        let props = segment_stream_plan_properties(&schema, Some(Index::from("log_time")), 4);

        assert_eq!(
            ordering_columns(&props),
            vec![format!("{segment_id}@0")],
            "expected segment-only ordering"
        );
        let datafusion::physical_expr::Partitioning::Hash(exprs, n) = props.partitioning else {
            panic!("expected hash partitioning, got {:?}", props.partitioning);
        };
        assert_eq!(n, 4);
        assert_eq!(exprs[0].to_string(), format!("{segment_id}@0"));
    }

    /// The `Partitioning::Hash` claim [`segment_stream_plan_properties`] makes
    /// is only true if [`segment_belongs_to_partition`] routes segments exactly
    /// the way `RepartitionExec` would. The random-state seed, the
    /// single-column `create_hashes` path, and the strength-reduced modulo are
    /// all DataFusion internals that can drift under us, and a divergence is
    /// silently wrong output for anything that trusts the claim — so pin the
    /// agreement against the real operator rather than re-deriving it here.
    ///
    /// The partition count is deliberately not a power of two: that is the case
    /// where the modulo actually divides instead of masking off low bits.
    ///
    /// This does *not* cover the wasm32 truncation described on
    /// [`segment_belongs_to_partition`] — on a 64-bit target `as usize` is
    /// lossless, so the narrowed and full-width forms agree here regardless.
    #[tokio::test]
    async fn segment_routing_matches_repartition_exec() {
        use arrow::array::Array as _;
        use datafusion::physical_plan::ExecutionPlan as _;
        use datafusion::physical_plan::repartition::RepartitionExec;
        use datafusion::prelude::SessionContext;
        use futures::StreamExt as _;

        const NUM_PARTITIONS: usize = 3;
        let name = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME;

        let ids = (0..64)
            .map(|i| format!("segment-{i:03}"))
            .collect::<Vec<_>>();
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new(name, DataType::Utf8, false)],
            Default::default(),
        ));
        let batch = RecordBatch::try_new_with_options(
            schema,
            vec![Arc::new(StringArray::from(ids.clone())) as ArrayRef],
            &RecordBatchOptions::default(),
        )
        .unwrap();

        let ctx = SessionContext::new();
        let source = ctx
            .read_batch(batch)
            .unwrap()
            .create_physical_plan()
            .await
            .unwrap();

        // Hand `RepartitionExec` the exact expression and partition count the
        // Hash claim declares, so the two sides cannot drift apart in the test.
        let physical = Arc::new(
            RepartitionExec::try_new(
                source,
                Partitioning::Hash(vec![Arc::new(Column::new(name, 0))], NUM_PARTITIONS),
            )
            .unwrap(),
        );

        assert_eq!(physical.partitioning().partition_count(), NUM_PARTITIONS);

        let mut routed = 0usize;
        for partition in 0..NUM_PARTITIONS {
            let mut stream = physical.execute(partition, ctx.task_ctx()).unwrap();
            while let Some(batch) = stream.next().await {
                let batch = batch.unwrap();
                let column = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("segment id column is Utf8");
                for row in 0..column.len() {
                    let segment_id = SegmentId::from(column.value(row));
                    assert!(
                        segment_belongs_to_partition(&segment_id, partition, NUM_PARTITIONS),
                        "RepartitionExec put {:?} in partition {partition}, we would not",
                        column.value(row)
                    );
                    routed += 1;
                }
            }
        }

        // Every row was seen exactly once, so the agreement is total and not
        // just vacuous on a subset.
        assert_eq!(routed, ids.len());
    }

    #[test]
    fn physical_column_by_name_declines_absent_and_ambiguous_names() {
        let schema = plan_props_schema(&["a", "b", "b"]);

        assert_eq!(
            physical_column_by_name(&schema, "a").map(|col| col.to_string()),
            Some("a@0".to_owned())
        );
        assert!(
            physical_column_by_name(&schema, "b").is_none(),
            "repeated field name must not resolve"
        );
        assert!(physical_column_by_name(&schema, "c").is_none());
    }

    /// A `RecordBatch` can only be built from a schema whose data types match the arrays
    /// exactly — arrow's `equals_datatype` compares child-field nullability. Fields keep their
    /// name, nullability and metadata; only the data type follows the array.
    #[test]
    fn schema_with_array_datatypes_follows_the_arrays() {
        use arrow::array::{ListArray, UInt8Array};
        use arrow::buffer::OffsetBuffer;

        let column = Arc::new(ListArray::new(
            Arc::new(Field::new_list_field(DataType::UInt8, false)),
            OffsetBuffer::from_lengths([2]),
            Arc::new(UInt8Array::from(vec![1u8, 2])),
            None,
        )) as ArrayRef;

        // The schema disagrees with the data about the item field's nullability, as a
        // `ChunkStore`-derived query schema does for an externally-produced column.
        let declared =
            Field::new("blob", DataType::new_list(DataType::UInt8, true), true).with_metadata(
                HashMap::from([("rerun:kind".to_owned(), "data".to_owned())]),
            );
        let schema = Schema::new_with_metadata(vec![declared.clone()], HashMap::default());

        let adjusted = schema_with_array_datatypes(&schema, &[Arc::clone(&column)]);
        let field = adjusted.field(0);

        assert_eq!(field.data_type(), column.data_type());
        assert_eq!(field.name(), declared.name());
        assert_eq!(field.is_nullable(), declared.is_nullable());
        assert_eq!(field.metadata(), declared.metadata());

        RecordBatch::try_new_with_options(
            Arc::new(adjusted),
            vec![Arc::clone(&column)],
            &RecordBatchOptions::default().with_row_count(Some(1)),
        )
        .expect("batch must build against the adjusted schema");

        // Matching data types are left alone.
        let matching = Schema::new_with_metadata(
            vec![Field::new("blob", column.data_type().clone(), true)],
            HashMap::default(),
        );
        assert_eq!(schema_with_array_datatypes(&matching, &[column]), matching);
    }

    #[test]
    fn test_batches_grouping() {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                ext::QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field_ref(),
                ext::QueryDatasetDataframe::COLUMN_CHUNK_ID.arrow_field_ref(),
            ],
            HashMap::default(),
        ));

        let capacity = 4;
        let byte_width = 16;
        let mut chunk_id_builder = FixedSizeBinaryBuilder::with_capacity(capacity, byte_width);
        chunk_id_builder.append_value([0u8; 16]).unwrap();
        chunk_id_builder.append_value([1u8; 16]).unwrap();
        chunk_id_builder.append_value([2u8; 16]).unwrap();
        chunk_id_builder.append_value([3u8; 16]).unwrap();
        let chunk_id_array = Arc::new(chunk_id_builder.finish());

        let batch1 = RecordBatch::try_new_with_options(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![
                    Some("A"),
                    Some("B"),
                    Some("A"),
                    Some("C"),
                ])),
                chunk_id_array,
            ],
            &RecordBatchOptions::new().with_row_count(Some(4)),
        )
        .unwrap();

        let mut chunk_id_builder = FixedSizeBinaryBuilder::with_capacity(capacity, byte_width);
        chunk_id_builder.append_value([4u8; 16]).unwrap();
        chunk_id_builder.append_value([5u8; 16]).unwrap();
        chunk_id_builder.append_value([6u8; 16]).unwrap();
        let chunk_id_array = Arc::new(chunk_id_builder.finish());

        let batch2 = RecordBatch::try_new_with_options(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![Some("B"), Some("C"), Some("D")])),
                chunk_id_array,
            ],
            &RecordBatchOptions::new().with_row_count(Some(3)),
        )
        .unwrap();

        let chunk_info_batches = Arc::new(vec![batch1, batch2]);

        let grouped = group_chunk_infos_by_segment_id(&chunk_info_batches).unwrap();

        assert_eq!(grouped.len(), 4);

        fn chunk_ids_of(batch: &RecordBatch) -> Vec<re_types_core::ChunkId> {
            QueryDatasetDataframe::COLUMN_CHUNK_ID
                .extract(batch)
                .unwrap()
                .to_vec()
        }

        let group_a = grouped.get("A").unwrap();
        assert_eq!(group_a.len(), 1);
        assert_eq!(
            chunk_ids_of(&group_a[0]),
            [[0u8; 16], [2u8; 16]].map(re_types_core::ChunkId::from)
        );

        let group_b = grouped.get("B").unwrap();
        assert_eq!(group_b.len(), 2);
        assert_eq!(
            chunk_ids_of(&group_b[0]),
            [[1u8; 16]].map(re_types_core::ChunkId::from)
        );
        assert_eq!(
            chunk_ids_of(&group_b[1]),
            [[4u8; 16]].map(re_types_core::ChunkId::from)
        );

        let group_c = grouped.get("C").unwrap();
        assert_eq!(group_c.len(), 2);
        assert_eq!(
            chunk_ids_of(&group_c[0]),
            [[3u8; 16]].map(re_types_core::ChunkId::from)
        );
        assert_eq!(
            chunk_ids_of(&group_c[1]),
            [[5u8; 16]].map(re_types_core::ChunkId::from)
        );

        let group_d = grouped.get("D").unwrap();
        assert_eq!(group_d.len(), 1);
        assert_eq!(
            chunk_ids_of(&group_d[0]),
            [[6u8; 16]].map(re_types_core::ChunkId::from)
        );
    }

    /// Build a synthetic chunk-info `RecordBatch` from parallel column vectors.
    fn make_chunk_info_batch(
        segment_ids: &[&str],
        layer_names: &[&str],
        byte_lens: &[u64],
    ) -> RecordBatch {
        use arrow::array::UInt64Array;

        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                ext::QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field_ref(),
                ext::QueryDatasetDataframe::COLUMN_RERUN_SEGMENT_LAYER.arrow_field_ref(),
                ext::QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN.arrow_field_ref(),
            ],
            HashMap::default(),
        ));

        let n = segment_ids.len();
        assert_eq!(n, layer_names.len());
        assert_eq!(n, byte_lens.len());

        RecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(StringArray::from(segment_ids.to_vec())),
                Arc::new(StringArray::from(layer_names.to_vec())),
                Arc::new(UInt64Array::from(byte_lens.to_vec())),
            ],
            &RecordBatchOptions::new().with_row_count(Some(n)),
        )
        .unwrap()
    }

    #[test]
    fn chunk_info_aggregates_empty() {
        let batch = make_chunk_info_batch(&[], &[], &[]);
        let agg = compute_chunk_info_aggregates(&batch);
        assert_eq!(agg.chunks, 0);
        assert_eq!(agg.segments, 0);
        assert_eq!(agg.layers, 0);
        assert_eq!(agg.bytes, 0);
        assert_eq!(agg.chunks_per_segment_min, 0);
        assert_eq!(agg.chunks_per_segment_max, 0);
        assert_eq!(agg.chunks_per_segment_mean, 0.0);
    }

    #[test]
    fn chunk_info_aggregates_single_segment() {
        // 3 chunks, all in segment "A", all in layer "base".
        let batch =
            make_chunk_info_batch(&["A", "A", "A"], &["base", "base", "base"], &[10, 20, 30]);
        let agg = compute_chunk_info_aggregates(&batch);
        assert_eq!(agg.chunks, 3);
        assert_eq!(agg.segments, 1);
        assert_eq!(agg.layers, 1);
        assert_eq!(agg.bytes, 60);
        assert_eq!(agg.chunks_per_segment_min, 3);
        assert_eq!(agg.chunks_per_segment_max, 3);
        assert!((agg.chunks_per_segment_mean - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn chunk_info_aggregates_uniform_segments() {
        // 6 chunks spread evenly: A,A | B,B | C,C.
        let batch = make_chunk_info_batch(
            &["A", "A", "B", "B", "C", "C"],
            &["base"; 6],
            &[1, 1, 1, 1, 1, 1],
        );
        let agg = compute_chunk_info_aggregates(&batch);
        assert_eq!(agg.chunks, 6);
        assert_eq!(agg.segments, 3);
        assert_eq!(agg.layers, 1);
        assert_eq!(agg.bytes, 6);
        assert_eq!(agg.chunks_per_segment_min, 2);
        assert_eq!(agg.chunks_per_segment_max, 2);
        assert!((agg.chunks_per_segment_mean - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn chunk_info_aggregates_skewed_segments() {
        // Sizes [1, 5, 10] — 16 chunks across 3 segments.
        let mut segs = vec!["A"];
        segs.extend(std::iter::repeat_n("B", 5));
        segs.extend(std::iter::repeat_n("C", 10));
        let layers = vec!["base"; segs.len()];
        let bytes = vec![1u64; segs.len()];

        let batch = make_chunk_info_batch(&segs, &layers, &bytes);
        let agg = compute_chunk_info_aggregates(&batch);
        assert_eq!(agg.chunks, 16);
        assert_eq!(agg.segments, 3);
        assert_eq!(agg.layers, 1);
        assert_eq!(agg.bytes, 16);
        assert_eq!(agg.chunks_per_segment_min, 1);
        assert_eq!(agg.chunks_per_segment_max, 10);
        // mean = 16/3 ≈ 5.333
        assert!((agg.chunks_per_segment_mean - (16.0 / 3.0)).abs() < 1e-5);
    }

    #[test]
    fn chunk_info_aggregates_multi_layer() {
        // Two segments, each touched in two layers — 4 distinct (segment, layer) rows.
        let batch = make_chunk_info_batch(
            &["A", "A", "B", "B"],
            &["base", "v2", "base", "v2"],
            &[100, 200, 300, 400],
        );
        let agg = compute_chunk_info_aggregates(&batch);
        assert_eq!(agg.chunks, 4);
        assert_eq!(agg.segments, 2);
        assert_eq!(agg.layers, 2);
        assert_eq!(agg.bytes, 1000);
        assert_eq!(agg.chunks_per_segment_min, 2);
        assert_eq!(agg.chunks_per_segment_max, 2);
        assert!((agg.chunks_per_segment_mean - 2.0).abs() < f32::EPSILON);
    }
}
