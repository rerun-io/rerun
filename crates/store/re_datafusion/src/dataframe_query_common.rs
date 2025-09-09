use std::any::Any;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr as _;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, DurationNanosecondArray, Int64Array, RecordBatch, StringArray,
    TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    TimestampSecondArray, UInt64Array, new_null_array,
};
use arrow::datatypes::{DataType, Field, Int64Type, Schema, SchemaRef, TimeUnit};
use arrow::record_batch::RecordBatchOptions;
use async_trait::async_trait;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::{Column, DataFusionError, downcast_value, exec_datafusion_err};
use datafusion::datasource::TableType;
use datafusion::logical_expr::{Expr, Operator, TableProviderFilterPushDown};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::physical_plan::coalesce_batches::CoalesceBatchesExec;

use re_dataframe::external::re_chunk::ChunkId;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::{Index, QueryExpression};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_log_types::external::re_types_core::Loggable as _;
use re_protos::cloud::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;
use re_protos::cloud::v1alpha1::ext::{Query, QueryLatestAt, QueryRange};
use re_protos::cloud::v1alpha1::{GetChunksRequest, GetDatasetSchemaRequest, QueryDatasetRequest};
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::{ConnectionClient, ConnectionRegistryHandle};
use re_sorbet::{BatchType, ChunkColumnDescriptors, ColumnKind, ComponentColumnSelector};
use re_tuid::Tuid;
use re_uri::Origin;

/// Sets the size for output record batches in rows. The last batch will likely be smaller.
/// The default for Data Fusion is 8192, which leads to a 256Kb record batch on average for
/// rows with 32b of data. We are setting this lower as a reasonable first guess to avoid
/// the pitfall of executing a single row at a time, but we will likely want to consider
/// at some point moving to a dynamic sizing.
const DEFAULT_BATCH_SIZE: usize = 2048;

#[derive(Debug)]
pub struct DataframeQueryTableProvider {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    sort_index: Option<Index>,
    chunk_info_batches: Arc<Vec<RecordBatch>>,
    client: ConnectionClient,
    chunk_request: GetChunksRequest,
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
        partition_ids: &[impl AsRef<str> + Sync],
    ) -> Result<Self, DataFusionError> {
        use futures::StreamExt as _;

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

        let query = query_from_query_expression(query_expression);

        let mut fields_of_interest = [
            "chunk_partition_id",
            "chunk_entity_path",
            "chunk_id",
            "chunk_is_static",
            "chunk_byte_len",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

        if let Some(index) = query_expression.filtered_index {
            fields_of_interest.push(format!("{}:start", index.as_str()));
            fields_of_interest.push(format!("{}:end", index.as_str()));
        }

        let chunk_request = GetChunksRequest {
            dataset_id: Some(dataset_id.into()),
            partition_ids: vec![],
            chunk_ids: vec![],
            entity_paths: entity_paths.iter().map(|p| (*p).clone().into()).collect(),
            select_all_entity_paths,
            fuzzy_descriptors: fuzzy_descriptors.clone(),
            exclude_static_data: false,
            exclude_temporal_data: false,
            query: Some(query.clone().into()),
        };

        let dataset_query = QueryDatasetRequest {
            partition_ids: partition_ids
                .iter()
                .map(|id| id.as_ref().to_owned().into())
                .collect(),
            chunk_ids: vec![],
            entity_paths: entity_paths
                .into_iter()
                .map(|p| (*p).clone().into())
                .collect(),
            select_all_entity_paths,
            fuzzy_descriptors,
            exclude_static_data: false,
            exclude_temporal_data: false,
            query: Some(query.into()),
            scan_parameters: Some(
                ScanParameters {
                    columns: fields_of_interest,
                    ..Default::default()
                }
                .into(),
            ),
        };

        let response_stream = client
            .inner()
            .query_dataset(
                tonic::Request::new(dataset_query)
                    .with_entry_id(dataset_id)
                    .map_err(|err| exec_datafusion_err!("{err}"))?,
            )
            .await
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_inner();

        let chunk_info_batches = response_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| exec_datafusion_err!("{err}"))?
            .into_iter()
            .filter_map(|response| response.data)
            .map(|dataframe_part| {
                dataframe_part
                    .decode()
                    .map_err(|err| exec_datafusion_err!("{err}"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into();

        let schema = Arc::new(prepend_string_column_schema(
            &schema,
            DATASET_MANIFEST_ID_FIELD_NAME,
        ));

        Ok(Self {
            schema,
            query_expression: query_expression.to_owned(),
            sort_index: query_expression.filtered_index,
            chunk_info_batches,
            client,
            chunk_request,
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
        let mut query_expression = self.query_expression.clone();

        // Find the first column selection that is a component
        if query_expression.filtered_is_not_null.is_none() {
            let filters = filters.iter().collect::<Vec<_>>();
            query_expression.filtered_is_not_null =
                Self::compute_column_is_neq_null_filter(&filters)
                    .into_iter()
                    .flatten()
                    .next();
        }

        crate::PartitionStreamExec::try_new(
            &self.schema,
            self.sort_index,
            projection,
            state.config().target_partitions(),
            Arc::clone(&self.chunk_info_batches),
            query_expression,
            self.client.clone(),
            self.chunk_request.clone(),
        )
        .map(Arc::new)
        .map(|exec| {
            Arc::new(CoalesceBatchesExec::new(exec, DEFAULT_BATCH_SIZE).with_fetch(limit))
                as Arc<dyn ExecutionPlan>
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
                .map(|filter| {
                    if Some(col) == filter.as_ref() {
                        TableProviderFilterPushDown::Exact
                    } else {
                        TableProviderFilterPushDown::Unsupported
                    }
                })
                .collect::<Vec<_>>())
        } else {
            Ok(vec![
                TableProviderFilterPushDown::Unsupported;
                filters.len()
            ])
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

    // When we call GetChunks we will not return row_id, so we only select indices and
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

#[tracing::instrument(level = "info", skip_all)]
pub fn align_record_batch_to_schema(
    batch: &RecordBatch,
    target_schema: &Arc<Schema>,
) -> Result<RecordBatch, DataFusionError> {
    let num_rows = batch.num_rows();

    let mut aligned_columns = Vec::with_capacity(target_schema.fields().len());

    for field in target_schema.fields() {
        if let Some((idx, _)) = batch.schema().column_with_name(field.name()) {
            let batch_data_type = batch.column(idx).data_type();
            if batch_data_type == &DataType::Null && field.data_type() != &DataType::Null {
                // Chunk store may output a null array of null data type
                aligned_columns.push(new_null_array(field.data_type(), num_rows));
            } else {
                aligned_columns.push(batch.column(idx).clone());
            }
        } else {
            // Fill with nulls of the right data type
            aligned_columns.push(new_null_array(field.data_type(), num_rows));
        }
    }

    Ok(RecordBatch::try_new_with_options(
        target_schema.clone(),
        aligned_columns,
        &RecordBatchOptions::new().with_row_count(Some(num_rows)),
    )?)
}

/// We need to create `num_partitions` of partition stream outputs, each of
/// which will be fed from multiple `rerun_partition_id` sources. The partitioning
/// output is a hash of the `rerun_partition_id`. We will reuse some of the
/// underlying execution code from `DataFusion`'s `RepartitionExec` to compute
/// these partition IDs, just to be certain they match partitioning generated
/// from sources other than Rerun gRPC services.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn compute_partition_stream_chunk_info(
    chunk_info_batches: &Arc<Vec<RecordBatch>>,
) -> Result<Arc<BTreeMap<String, Vec<ChunkInfo>>>, DataFusionError> {
    let mut results = BTreeMap::new();

    for batch in chunk_info_batches.as_ref() {
        // TODO(tsaucer) see comment below
        // let schema = batch.schema();
        // let end_time_col = schema
        //     .fields()
        //     .iter()
        //     .map(|f| f.name())
        //     .find(|name| name.ends_with(":end"))
        //     .ok_or(exec_datafusion_err!("Unable to identify time index"))?;
        // let start_time_col = schema
        //     .fields()
        //     .iter()
        //     .map(|f| f.name())
        //     .find(|name| name.ends_with(":start"))
        //     .ok_or(exec_datafusion_err!("Unable to identify time index"))?;

        let partition_id_arr = batch
            .column_by_name("chunk_partition_id")
            .ok_or(exec_datafusion_err!(
                "Unable to return chunk_partition_id as expected"
            ))?
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(exec_datafusion_err!("Unexpected type for chunk_id"))?;

        let chunk_id_arr = batch
            .column_by_name("chunk_id")
            .ok_or(exec_datafusion_err!(
                "Unable to return chunk_id as expected"
            ))
            .and_then(|arr| Tuid::from_arrow(arr).map_err(|err| exec_datafusion_err!("{err}")))?;

        let chunk_byte_len_arr = batch
            .column_by_name("chunk_byte_len")
            .ok_or(exec_datafusion_err!(
                "Unable to return chunk_byte_len as expected"
            ))?
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or(exec_datafusion_err!("Unexpected type for chunk_id"))?;

        // TODO(tsaucer) uncomment and ensure this can still work with no timeline selected
        // Below we are setting the start time to the index, because we are not yet
        // processing the times for when it is okay to send the next row.

        // let end_time_arr = batch
        //     .column_by_name(end_time_col)
        //     .ok_or(exec_datafusion_err!(
        //         "Unable to return end time column as expected"
        //     ))?;
        // let end_time_arr = time_array_ref_to_i64(end_time_arr)?;
        // let start_time_arr = batch
        //     .column_by_name(start_time_col)
        //     .ok_or(exec_datafusion_err!(
        //         "Unable to return start time column as expected"
        //     ))?;
        // let start_time_arr = time_array_ref_to_i64(start_time_arr)?;

        for (idx, chunk_id) in chunk_id_arr.iter().enumerate() {
            let partition_id = partition_id_arr.value(idx).to_owned();
            let chunk_id = ChunkId::from_tuid(*chunk_id);
            let byte_len = chunk_byte_len_arr.value(idx);
            let start_time = idx as i64;
            let end_time = idx as i64 + 1;

            let chunk_info = ChunkInfo {
                start_time,
                end_time,
                chunk_id,
                byte_len,
            };

            let chunks_vec = results.entry(partition_id).or_insert(vec![]);
            chunks_vec.push(chunk_info);
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
        columns_always_include_chunk_ids: false,
        columns_always_include_entity_paths: false,
        columns_always_include_byte_offsets: false,
        columns_always_include_static_indexes: false,
        columns_always_include_global_indexes: false,
        columns_always_include_component_indexes: false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChunkInfo {
    pub start_time: i64,
    pub end_time: i64,
    pub chunk_id: ChunkId,
    pub byte_len: u64,
}

impl Ord for ChunkInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        let start_time_cmp = self.start_time.cmp(&other.start_time);
        let Ordering::Equal = start_time_cmp else {
            return start_time_cmp;
        };
        let end_time_cmp = self.end_time.cmp(&other.end_time);
        let Ordering::Equal = end_time_cmp else {
            return end_time_cmp;
        };
        let chunk_id_cmp = self.chunk_id.cmp(&other.chunk_id);
        let Ordering::Equal = chunk_id_cmp else {
            return chunk_id_cmp;
        };

        // We should never get here
        self.byte_len.cmp(&other.byte_len)
    }
}

impl PartialOrd for ChunkInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
