use crate::batch_coalescer::coalesce_exec::SizedCoalesceBatchesExec;
use crate::batch_coalescer::coalescer::CoalescerOptions;
use crate::pushdown_expressions::{apply_filter_expr_to_queries, filter_expr_is_supported};
use ahash::HashSet;
use arrow::array::{
    Array as _, ArrayRef, DurationNanosecondArray, FixedSizeBinaryArray, Int64Array, RecordBatch,
    StringArray, TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    TimestampSecondArray, UInt32Array,
};
use arrow::compute::concat_batches;
use arrow::datatypes::{DataType, Field, Int64Type, Schema, SchemaRef, TimeUnit};
use arrow::record_batch::RecordBatchOptions;
use async_trait::async_trait;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::{Column, DataFusionError, downcast_value, exec_datafusion_err};
use datafusion::datasource::TableType;
use datafusion::logical_expr::{Expr, Operator, TableProviderFilterPushDown};
use datafusion::physical_plan::ExecutionPlan;
use futures::StreamExt as _;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::{Index, QueryExpression};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{Query, QueryDatasetRequest, QueryLatestAt, QueryRange};
use re_protos::cloud::v1alpha1::{
    FetchChunksRequest, GetDatasetSchemaRequest, QueryDatasetResponse, ScanSegmentTableResponse,
};
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::{ConnectionClient, ConnectionRegistryHandle};
use re_sorbet::{BatchType, ChunkColumnDescriptors, ColumnKind, ComponentColumnSelector};
use re_uri::Origin;
use std::any::Any;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr as _;
use std::sync::Arc;

/// Sets the size for output record batches in rows. The last batch will likely be smaller.
/// The default for Data Fusion is 8192, which leads to a 256Kb record batch on average for
/// rows with 32b of data. We are setting this lower as a reasonable first guess to avoid
/// the pitfall of executing a single row at a time, but we will likely want to consider
/// at some point moving to a dynamic sizing.
const DEFAULT_BATCH_BYTES: u64 = 200 * 1024 * 1024;
const DEFAULT_BATCH_ROWS: usize = 2048;

#[derive(Debug)]
pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    query_dataset_request: QueryDatasetRequest,
    sort_index: Option<Index>,
    dataset_id: EntryId,
    client: ConnectionClient,

    /// passing trace headers between phases of execution pipeline helps keep
    /// the entire operation under a single trace.
    #[cfg(not(target_arch = "wasm32"))]
    trace_headers: Option<crate::TraceHeaders>,
}

impl DataframeQueryTableProvider {
    /// Create a table provider for a gRPC query. This function is async
    /// because we need to make gRPC calls to determine the schema at the
    /// creation of the table provider.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn new(
        origin: Origin,
        connection: ConnectionRegistryHandle,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
        segment_ids: &[impl AsRef<str> + Sync],
        #[cfg(not(target_arch = "wasm32"))] trace_headers: Option<crate::TraceHeaders>,
    ) -> Result<Self, DataFusionError> {
        let mut client = connection
            .client(origin)
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        let schema = client
            .inner()
            .get_dataset_schema(
                tonic::Request::new(GetDatasetSchemaRequest {})
                    .with_entry_id(dataset_id)
                    .map_err(|err| exec_datafusion_err!("{err}"))?,
            )
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_inner()
            .schema()
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        let schema = compute_schema_for_query(&schema, query_expression)?;

        let select_all_entity_paths = false;

        let entity_paths = query_expression
            .view_contents
            .as_ref()
            .map_or(vec![], |contents| contents.keys().collect::<Vec<_>>());

        let query = query_from_query_expression(query_expression);
        let fuzzy_descriptors: Vec<String> = query_expression
            .view_contents
            .as_ref()
            .map_or(BTreeSet::new(), |contents| {
                contents
                    .values()
                    .filter_map(|opt_set| opt_set.as_ref())
                    .flat_map(|set| set.iter().copied())
                    .collect::<BTreeSet<_>>()
            })
            .into_iter()
            .map(|ident| ident.to_string())
            .collect();

        let query_dataset_request = QueryDatasetRequest {
            segment_ids: segment_ids
                .iter()
                .map(|id| id.as_ref().to_owned().into())
                .collect(),
            chunk_ids: vec![],
            entity_paths: entity_paths.into_iter().map(|p| (*p).clone()).collect(),
            select_all_entity_paths,
            fuzzy_descriptors,
            exclude_static_data: false,
            exclude_temporal_data: false,
            query: Some(query),
            scan_parameters: Some(ScanParameters {
                columns: FetchChunksRequest::required_column_names(),
                ..Default::default()
            }),
        };

        let schema = Arc::new(prepend_string_column_schema(
            &schema,
            ScanSegmentTableResponse::FIELD_SEGMENT_ID,
        ));

        Ok(Self {
            schema,
            query_expression: query_expression.to_owned(),
            query_dataset_request,
            sort_index: query_expression.filtered_index,
            dataset_id,
            client,
            #[cfg(not(target_arch = "wasm32"))]
            trace_headers,
        })
    }

    fn selector_from_column(column: &Column) -> Option<ComponentColumnSelector> {
        ComponentColumnSelector::from_str(column.name()).ok()
    }

    fn is_neq_null(expr: &Expr) -> Option<&Column> {
        match expr {
            Expr::IsNotNull(inner) => {
                if let Expr::Column(col) = inner.as_ref() {
                    return Some(col);
                }
            }
            Expr::Not(inner) => {
                if let Expr::IsNull(col_expr) = inner.as_ref()
                    && let Expr::Column(col) = col_expr.as_ref()
                {
                    return Some(col);
                }
            }
            Expr::BinaryExpr(binary) => {
                if binary.op == Operator::NotEq
                    && let (Expr::Column(col), Expr::Literal(sv, _))
                    | (Expr::Literal(sv, _), Expr::Column(col)) =
                        (binary.left.as_ref(), binary.right.as_ref())
                    && sv.is_null()
                {
                    return Some(col);
                }
            }
            _ => {}
        }

        None
    }

    /// For a given input expression, check to see if it can match the supported
    /// row filtering. We can currently filter out rows for which a specific
    /// component of one entity is not null. We do this by checking the column
    /// name matches the entity path and component naming conventions, which
    /// should always be true at the level of this call. We attempt to match
    /// a few different logically equivalent variants the user may pass.
    fn compute_column_is_neq_null_filter(
        filters: &[&Expr],
    ) -> Vec<Option<ComponentColumnSelector>> {
        filters
            .iter()
            .map(|expr| Self::is_neq_null(expr).and_then(Self::selector_from_column))
            .collect()
    }
}

#[async_trait]
impl TableProvider for DataframeQueryTableProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    #[tracing::instrument(level = "info", skip_all)]
    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        let mut dataset_queries = vec![self.query_dataset_request.clone()];
        for filter in filters {
            if let Some(updated_queries) =
                apply_filter_expr_to_queries(dataset_queries.clone(), filter, &self.schema)?
            {
                dataset_queries = updated_queries;
            }
        }

        let mut query_expression = self.query_expression.clone();

        let mut chunk_info_batches = Vec::with_capacity(dataset_queries.len());
        for dataset_query in dataset_queries {
            let response_stream = self
                .client
                .clone()
                .inner()
                .query_dataset(
                    tonic::Request::new(dataset_query.into())
                        .with_entry_id(self.dataset_id)
                        .map_err(|err| exec_datafusion_err!("{err}"))?,
                )
                .await
                .map_err(|err| exec_datafusion_err!("{err}"))?
                .into_inner();

            let batches: Vec<RecordBatch> = response_stream
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| exec_datafusion_err!("{err}"))?
                .into_iter()
                .filter_map(|response| response.data)
                .map(|dataframe_part| {
                    dataframe_part
                        .try_into()
                        .map_err(|err| exec_datafusion_err!("{err}"))
                })
                .collect::<Result<Vec<_>, _>>()?;

            chunk_info_batches.push(batches);
        }
        let chunk_info_batches = Arc::new(compute_unique_chunk_info_ids(chunk_info_batches)?);

        // Find the first column selection that is a component
        if query_expression.filtered_is_not_null.is_none() {
            let filters = filters.iter().collect::<Vec<_>>();
            query_expression.filtered_is_not_null =
                Self::compute_column_is_neq_null_filter(&filters)
                    .into_iter()
                    .flatten()
                    .next();
        }

        crate::SegmentStreamExec::try_new(
            &self.schema,
            self.sort_index,
            projection,
            state.config().target_partitions(),
            chunk_info_batches,
            query_expression,
            self.client.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            self.trace_headers.clone(),
        )
        .map(Arc::new)
        .map(|exec| {
            Arc::new(SizedCoalesceBatchesExec::new(
                exec,
                CoalescerOptions {
                    target_batch_rows: DEFAULT_BATCH_ROWS,
                    target_batch_bytes: DEFAULT_BATCH_BYTES,
                    max_rows: limit,
                },
            )) as Arc<dyn ExecutionPlan>
        })
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> datafusion::common::Result<Vec<TableProviderFilterPushDown>> {
        let filter_columns = Self::compute_column_is_neq_null_filter(filters);
        let non_null_columns = filter_columns.iter().flatten().collect::<Vec<_>>();
        if let Some(col) = non_null_columns.first() {
            let col = *col;
            Ok(filter_columns
                .iter()
                .zip(filters)
                .map(|(column_selector, filter_expr)| {
                    if Some(col) == column_selector.as_ref() {
                        Ok(TableProviderFilterPushDown::Exact)
                    } else {
                        filter_expr_is_supported(
                            filter_expr,
                            &self.query_dataset_request,
                            &self.schema,
                        )
                    }
                })
                .collect::<Result<Vec<_>, DataFusionError>>()?)
        } else {
            Ok(filters
                .iter()
                .map(|filter_expr| {
                    filter_expr_is_supported(filter_expr, &self.query_dataset_request, &self.schema)
                })
                .collect::<Result<Vec<_>, DataFusionError>>()?)
        }
    }
}

/// Compute the output schema for a query on a dataset. When we call `get_dataset_schema`
/// on the data platform, we will get the schema for all entities and all components. This
/// method is used to down select from that full schema based on `query_expression`.
#[tracing::instrument(level = "trace", skip_all)]
fn compute_schema_for_query(
    dataset_schema: &Schema,
    query_expression: &QueryExpression,
) -> Result<SchemaRef, DataFusionError> {
    // Short circuit for empty datasets. Needed because `ChunkColumnDescriptors::try_from_arrow_fields`
    // needs row ids, which we only have for non-empty datasets.
    if dataset_schema.fields.is_empty() {
        return Ok(Arc::new(Schema::empty()));
    }

    // Schema returned from `get_dataset_schema` does not match the required ChunkColumnDescriptors ordering
    // which is row id, then time, then data. We don't need perfect ordering other than that.
    let mut fields = dataset_schema
        .fields()
        .iter()
        .map(Arc::clone)
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| {
        let Ok(a) = ColumnKind::try_from(a.as_ref()) else {
            return Ordering::Equal;
        };
        let Ok(b) = ColumnKind::try_from(b.as_ref()) else {
            return Ordering::Equal;
        };

        match (a, b) {
            (ColumnKind::RowId, _) => Ordering::Less,
            (_, ColumnKind::RowId) => Ordering::Greater,
            (ColumnKind::Index, _) => Ordering::Less,
            (_, ColumnKind::Index) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });
    let fields: arrow::datatypes::Fields = fields.into();

    let column_descriptors = ChunkColumnDescriptors::try_from_arrow_fields(None, &fields)
        .map_err(|err| exec_datafusion_err!("col desc {err}"))?;

    // Create the actual filter to apply to the column descriptors
    let filter = ChunkStore::create_component_filter_from_query(query_expression);

    // When we call QueryDataset we will not return row_id, so we only select indices and
    // components from the column descriptors.
    let filtered_fields = column_descriptors
        .filter_components(filter)
        .indices_and_components()
        .into_iter()
        .map(|cd| cd.to_arrow_field(BatchType::Dataframe))
        .collect::<Vec<_>>();

    Ok(Arc::new(Schema::new_with_metadata(
        filtered_fields,
        dataset_schema.metadata().clone(),
    )))
}

#[tracing::instrument(level = "info", skip_all)]
pub(crate) fn prepend_string_column_schema(schema: &Schema, column_name: &str) -> Schema {
    let mut fields = vec![Field::new(column_name, DataType::Utf8, false)];
    fields.extend(schema.fields().iter().map(|f| (**f).clone()));
    Schema::new_with_metadata(fields, schema.metadata.clone())
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
    chunk_info_batches: &Arc<Vec<RecordBatch>>,
) -> Result<Arc<BTreeMap<String, Vec<RecordBatch>>>, DataFusionError> {
    let mut results = BTreeMap::new();

    for batch in chunk_info_batches.as_ref() {
        let segment_ids = batch
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID)
            .ok_or(exec_datafusion_err!(
                "Unable to find {} column",
                QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID
            ))?
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(exec_datafusion_err!(
                "{} must be string type",
                QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID
            ))?;

        // group rows by segment ID
        let mut segment_rows: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (row_idx, segment_id) in segment_ids.iter().enumerate() {
            let sid = segment_id.ok_or(exec_datafusion_err!(
                "Found null segment id in {} column at row {row_idx}",
                QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID
            ))?;
            segment_rows
                .entry(sid.to_owned())
                .or_default()
                .push(row_idx);
        }

        for (segment_id, row_indices) in segment_rows {
            if row_indices.is_empty() {
                continue;
            }

            // Create indices array for take operation
            let indices = arrow::array::UInt32Array::from(
                row_indices.iter().map(|&i| i as u32).collect::<Vec<_>>(),
            );
            let segment_batch = arrow::compute::take_record_batch(batch, &indices)?;

            results
                .entry(segment_id)
                .or_insert_with(Vec::new)
                .push(segment_batch);
        }
    }

    Ok(Arc::new(results))
}

#[tracing::instrument(level = "trace", skip_all)]
#[expect(dead_code)]
pub(crate) fn time_array_ref_to_i64(time_array: &ArrayRef) -> Result<Int64Array, DataFusionError> {
    Ok(match time_array.data_type() {
        DataType::Int64 => downcast_value!(time_array, Int64Array).reinterpret_cast::<Int64Type>(),
        DataType::Timestamp(TimeUnit::Second, _) => {
            let nano_array = downcast_value!(time_array, TimestampSecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            let nano_array = downcast_value!(time_array, TimestampMillisecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            let nano_array = downcast_value!(time_array, TimestampMicrosecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            let nano_array = downcast_value!(time_array, TimestampNanosecondArray);
            nano_array.reinterpret_cast::<Int64Type>()
        }
        DataType::Duration(TimeUnit::Nanosecond) => {
            let duration_array = downcast_value!(time_array, DurationNanosecondArray);
            duration_array.reinterpret_cast::<Int64Type>()
        }
        _ => {
            return Err(exec_datafusion_err!(
                "Unexpected type for time column {}",
                time_array.data_type()
            ));
        }
    })
}

pub fn query_from_query_expression(query_expression: &QueryExpression) -> Query {
    let latest_at = if query_expression.is_static() {
        Some(QueryLatestAt::new_static())
    } else {
        query_expression
            .min_latest_at()
            .map(|latest_at| QueryLatestAt {
                index: Some(latest_at.timeline().to_string()),
                at: latest_at.at(),
            })
    };

    Query {
        latest_at,
        range: query_expression.max_range().map(|range| QueryRange {
            index: range.timeline().to_string(),
            index_range: range.range,
        }),
        columns_always_include_everything: false,
        columns_always_include_entity_paths: false,
        columns_always_include_byte_offsets: false,
        columns_always_include_static_indexes: false,
        columns_always_include_global_indexes: false,
        columns_always_include_component_indexes: false,
    }
}

fn compute_unique_chunk_info_ids(
    chunk_info_batches: Vec<Vec<RecordBatch>>,
) -> Result<Vec<RecordBatch>, DataFusionError> {
    let batches: Vec<_> = chunk_info_batches.into_iter().flatten().collect();
    if batches.is_empty() {
        return Ok(vec![]);
    }

    let schema = batches[0].schema();
    let combined = concat_batches(&schema, &batches)?;

    // Find the chunk_id column
    let chunk_id_col = combined
        .column_by_name("chunk_id")
        .ok_or(exec_datafusion_err!("chunk_id column not found"))?;

    let chunk_id_array = chunk_id_col
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .ok_or(exec_datafusion_err!("chunk_id is not FixedSizeBinary"))?;

    let mut indices_to_keep = Vec::new();
    let mut seen: HashSet<[u8; 16]> = HashSet::default();

    for row_idx in 0..combined.num_rows() {
        let chunk_id = chunk_id_array.value(row_idx);
        let chunk_id_fixed: [u8; 16] = chunk_id
            .try_into()
            .expect("chunk_id should be exactly 16 bytes");

        if seen.insert(chunk_id_fixed) {
            indices_to_keep.push(row_idx as u32);
        }
    }

    let indices = UInt32Array::from(indices_to_keep);

    let distinct_columns = arrow::compute::take_arrays(combined.columns(), &indices, None)?;

    Ok(vec![RecordBatch::try_new_with_options(
        schema,
        distinct_columns,
        &RecordBatchOptions::default(),
    )?])
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow::array::{Array as _, FixedSizeBinaryArray, FixedSizeBinaryBuilder};

    use super::*;

    #[test]
    fn test_batches_grouping() {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                QueryDatasetResponse::field_chunk_segment_id(),
                QueryDatasetResponse::field_chunk_id(),
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

        let group_a = grouped.get("A").unwrap();
        assert_eq!(group_a.len(), 1);
        let chunk_ids_a = group_a[0]
            .column_by_name("chunk_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_a.len(), 2);
        assert_eq!(chunk_ids_a.value(0), [0u8; 16]);
        assert_eq!(chunk_ids_a.value(1), [2u8; 16]);

        let group_b = grouped.get("B").unwrap();
        assert_eq!(group_b.len(), 2);
        let chunk_ids_b1 = group_b[0]
            .column_by_name("chunk_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_b1.len(), 1);
        assert_eq!(chunk_ids_b1.value(0), [1u8; 16]);
        let chunk_ids_b2 = group_b[1]
            .column_by_name("chunk_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_b2.len(), 1);
        assert_eq!(chunk_ids_b2.value(0), [4u8; 16]);

        let group_c = grouped.get("C").unwrap();
        assert_eq!(group_c.len(), 2);
        let chunk_ids_c1 = group_c[0]
            .column_by_name("chunk_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_c1.len(), 1);
        assert_eq!(chunk_ids_c1.value(0), [3u8; 16]);
        let chunk_ids_c2 = group_c[1]
            .column_by_name("chunk_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_c2.len(), 1);
        assert_eq!(chunk_ids_c2.value(0), [5u8; 16]);

        let group_d = grouped.get("D").unwrap();
        assert_eq!(group_d.len(), 1);
        let chunk_ids_d = group_d[0]
            .column_by_name("chunk_id")
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_d.len(), 1);
        assert_eq!(chunk_ids_d.value(0), [6u8; 16]);
    }
}
