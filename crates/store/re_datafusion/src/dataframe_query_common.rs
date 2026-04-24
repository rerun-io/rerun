use crate::analytics::QueryType;
use crate::batch_coalescer::coalesce_exec::SizedCoalesceBatchesExec;
use crate::batch_coalescer::coalescer::CoalescerOptions;
use crate::pushdown_expressions::{apply_filter_expr_to_queries, filter_expr_is_supported};
use ahash::{HashMap, HashMapExt as _, HashSet};
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
use re_dataframe::{Index, IndexValue, QueryExpression, SparseFillStrategy};
use re_log_types::{EntityPath, EntryId};
use re_protos::cloud::v1alpha1::ext::{Query, QueryDatasetRequest, QueryLatestAt, QueryRange};
use re_protos::cloud::v1alpha1::{
    FetchChunksRequest, GetDatasetSchemaRequest, GetDatasetSchemaResponse, QueryDatasetResponse,
    ScanSegmentTableResponse,
};
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::{ApiError, ApiResult, ConnectionClient, ConnectionRegistryHandle};

use crate::IntoDfError as _;
use re_sorbet::{BatchType, ChunkColumnDescriptors, ColumnKind, ComponentColumnSelector};
use re_uri::Origin;
use std::any::Any;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr as _;
use std::sync::Arc;
use tracing::Instrument as _;
use web_time::Instant;

/// Sets the size for output record batches in rows. The last batch will likely be smaller.
/// The default for Data Fusion is 8192, which leads to a 256Kb record batch on average for
/// rows with 32b of data. We are setting this lower as a reasonable first guess to avoid
/// the pitfall of executing a single row at a time, but we will likely want to consider
/// at some point moving to a dynamic sizing.
pub(crate) const DEFAULT_BATCH_BYTES: u64 = 200 * 1024 * 1024;
pub(crate) const DEFAULT_BATCH_ROWS: usize = 2048;

/// Mapping of `rerun_segment_id` to set of `IndexValues` to be used for querying
/// a specific set of index values per segment. If the option is None, then
/// `using_index_values` will not be applied to the dataset queries.
pub(crate) type IndexValuesMap = Option<Arc<BTreeMap<String, BTreeSet<IndexValue>>>>;

#[derive(Debug)]
pub struct DataframeQueryTableProvider<T: DataframeClientAPI> {
    pub schema: SchemaRef,
    query_expression: QueryExpression,
    query_dataset_request: QueryDatasetRequest,
    sort_index: Option<Index>,
    dataset_id: EntryId,
    client: T,
    index_values: IndexValuesMap,

    /// passing trace headers between phases of execution pipeline helps keep
    /// the entire operation under a single trace.
    #[cfg(not(target_arch = "wasm32"))]
    trace_headers: Option<crate::TraceHeaders>,

    /// Per-connection analytics sender for query stats.
    analytics: Option<crate::ConnectionAnalytics>,
}

/// This trait provides the specific methods used when interacting with the
/// gRPC services for the datafusion client services. By implementing this
/// as a trait we can provide an alternative implementation in our testing
/// facility to remove all gRPC layers and test the server responses
/// more directly.
#[async_trait]
pub trait DataframeClientAPI: std::fmt::Debug + Clone + Send + Sync + Unpin + 'static {
    async fn get_dataset_schema(
        &mut self,
        request: tonic::Request<GetDatasetSchemaRequest>,
    ) -> tonic::Result<tonic::Response<GetDatasetSchemaResponse>>;

    async fn query_dataset(
        &mut self,
        request: tonic::Request<re_protos::cloud::v1alpha1::QueryDatasetRequest>,
    ) -> tonic::Result<tonic::Response<tonic::codec::Streaming<QueryDatasetResponse>>>;

    async fn fetch_chunks(
        &mut self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FetchChunksRequest>,
    ) -> tonic::Result<
        tonic::Response<tonic::codec::Streaming<re_protos::cloud::v1alpha1::FetchChunksResponse>>,
    >;
}

#[async_trait]
impl DataframeClientAPI for ConnectionClient {
    async fn get_dataset_schema(
        &mut self,
        request: tonic::Request<GetDatasetSchemaRequest>,
    ) -> tonic::Result<tonic::Response<GetDatasetSchemaResponse>> {
        self.inner().get_dataset_schema(request).await
    }

    async fn query_dataset(
        &mut self,
        request: tonic::Request<re_protos::cloud::v1alpha1::QueryDatasetRequest>,
    ) -> tonic::Result<tonic::Response<tonic::codec::Streaming<QueryDatasetResponse>>> {
        self.inner().query_dataset(request).await
    }

    async fn fetch_chunks(
        &mut self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FetchChunksRequest>,
    ) -> tonic::Result<
        tonic::Response<tonic::codec::Streaming<re_protos::cloud::v1alpha1::FetchChunksResponse>>,
    > {
        self.inner().fetch_chunks(request).await
    }
}

impl DataframeQueryTableProvider<ConnectionClient> {
    /// Create a table provider for a gRPC query. This function is async
    /// because we need to make gRPC calls to determine the schema at the
    /// creation of the table provider.
    ///
    /// If `arrow_schema` is `Some`, it is used directly and the `/GetDatasetSchema`
    /// RPC is skipped — useful when the caller has already fetched the schema.
    #[tracing::instrument(level = "info", skip_all)]
    #[cfg_attr(not(target_arch = "wasm32"), expect(clippy::too_many_arguments))]
    pub async fn new(
        origin: Origin,
        connection: ConnectionRegistryHandle,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
        segment_ids: &[impl AsRef<str> + Sync],
        index_values: IndexValuesMap,
        arrow_schema: Option<Schema>,
        #[cfg(not(target_arch = "wasm32"))] trace_headers: Option<crate::TraceHeaders>,
    ) -> ApiResult<Self> {
        let client = connection.client(origin.clone()).await?;

        let mut provider = Self::new_from_client(
            client,
            dataset_id,
            query_expression,
            segment_ids,
            index_values,
            arrow_schema,
            #[cfg(not(target_arch = "wasm32"))]
            trace_headers,
        )
        .await?;

        let analytics = crate::ConnectionAnalytics::new(origin);

        // Kick off a background fetch of the server version so subsequent analytics
        // spans can be filtered by cloud build. Lazy-cached on `analytics`; the
        // first query will ship without it, the rest will have it.
        {
            let analytics_bg = analytics.clone();
            let mut client_bg = provider.client.clone();
            let fetch_fut = async move {
                match client_bg.version_info().await {
                    Ok(response) => {
                        analytics_bg.set_server_version(Some(response.version));
                    }
                    Err(err) => {
                        re_log::debug_once!("Failed to fetch server version for analytics: {err}");
                        analytics_bg.set_server_version(None);
                    }
                }
            };

            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(fetch_fut);

            #[cfg(not(target_arch = "wasm32"))]
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(fetch_fut);
            }
        }

        provider.analytics = Some(analytics);

        Ok(provider)
    }
}

impl<T: DataframeClientAPI> DataframeQueryTableProvider<T> {
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn new_from_client(
        mut client: T,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
        segment_ids: &[impl AsRef<str> + Sync],
        index_values: IndexValuesMap,
        arrow_schema: Option<Schema>,
        #[cfg(not(target_arch = "wasm32"))] trace_headers: Option<crate::TraceHeaders>,
    ) -> ApiResult<Self> {
        // Either use the caller-provided schema or fetch it from the server.
        let (schema, trace_id) = if let Some(schema) = arrow_schema {
            (schema, None)
        } else {
            let request = tonic::Request::new(GetDatasetSchemaRequest {})
                .with_entry_id(dataset_id)
                .map_err(|err| {
                    ApiError::internal_with_source(None, err, "attaching dataset entry_id header")
                })?;
            let response = client
                .get_dataset_schema(request)
                .await
                .map_err(|err| ApiError::tonic(err, "get_dataset_schema"))?;
            let trace_id = re_redap_client::extract_trace_id(response.metadata());
            let schema = response.into_inner().schema().map_err(|err| {
                ApiError::deserialization_with_source(trace_id, err, "decoding dataset schema")
            })?;
            (schema, trace_id)
        };

        let schema = compute_schema_for_query(&schema, query_expression).map_err(|err| {
            // `compute_schema_for_query` fails when the caller-provided query
            // references columns/entity-paths not present in the dataset schema
            ApiError::invalid_arguments_with_source(trace_id, err, "computing schema for query")
        })?;

        let select_all_entity_paths = false;

        let entity_paths = query_expression
            .view_contents
            .as_ref()
            .map_or(vec![], |contents| {
                contents.keys().cloned().collect::<Vec<_>>()
            });

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
            entity_paths,
            select_all_entity_paths,
            fuzzy_descriptors,
            exclude_static_data: false,
            exclude_temporal_data: false,
            query: Some(query),
            scan_parameters: Some(ScanParameters {
                columns: FetchChunksRequest::required_column_names(),
                ..Default::default()
            }),
            generate_direct_urls: true,
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
            index_values,
            #[cfg(not(target_arch = "wasm32"))]
            trace_headers,
            analytics: None,
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
impl<T: DataframeClientAPI> TableProvider for DataframeQueryTableProvider<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        let scan_span = {
            // Attach trace context BEFORE creating the span so the span is
            // parented under the propagated trace
            #[cfg(not(target_arch = "wasm32"))]
            let _trace_guard =
                crate::dataframe_query_provider::attach_trace_context(self.trace_headers.as_ref());

            tracing::info_span!("scan")
        };

        async {
            let scan_start_wall = web_time::SystemTime::now();
            let scan_start = Instant::now();

            let mut dataset_queries = vec![self.query_dataset_request.clone()];
            for filter in filters {
                if let Some(updated_queries) =
                    apply_filter_expr_to_queries(dataset_queries.clone(), filter, &self.schema)?
                {
                    dataset_queries = updated_queries;
                }
            }

            // Entity path projection pushdown: narrow the server request to only
            // fetch chunks for entity paths that are actually needed by the projection
            // and filters. Skip when fill_latest_at is enabled, because timestamps
            // from excluded entities would produce rows with filled values that the
            // user expects.
            if self.query_expression.sparse_fill_strategy == SparseFillStrategy::None
                && let Some(projected_paths) = projection.map(|projection| {
                    extract_projected_entity_paths(&self.schema, projection, filters)
                })
                && !projected_paths.is_empty()
            {
                for query in &mut dataset_queries {
                    if !query.select_all_entity_paths && !query.entity_paths.is_empty() {
                        query
                            .entity_paths
                            .retain(|path| projected_paths.contains(path));
                    }
                }
            }

            let mut query_expression = self.query_expression.clone();

            let mut chunk_info_batches = Vec::with_capacity(dataset_queries.len());
            let mut time_to_first_chunk_info: Option<std::time::Duration> = None;

            let mut trace_id: Option<opentelemetry::TraceId> = None;

            for dataset_query in dataset_queries {
                let query_start = Instant::now();

                let request = tonic::Request::new(dataset_query.into())
                    .with_entry_id(self.dataset_id)
                    .map_err(|err| {
                        ApiError::internal_with_source(
                            None,
                            err,
                            "attaching dataset entry_id header",
                        )
                        .into_df_error()
                    })?;
                let response = self
                    .client
                    .clone()
                    .query_dataset(request)
                    .await
                    .map_err(|err| ApiError::tonic(err, "query_dataset").into_df_error())?;

                // Capture the server-side trace-id from response metadata.
                if trace_id.is_none() {
                    trace_id = re_redap_client::extract_trace_id(response.metadata());
                }

                let mut response_stream = response.into_inner();

                while let Some(response) = response_stream.next().await {
                    if time_to_first_chunk_info.is_none() {
                        time_to_first_chunk_info = Some(query_start.elapsed());
                    }

                    let response = response.map_err(|err| {
                        ApiError::tonic(err, "query_dataset response stream")
                            .with_trace_id(trace_id)
                            .into_df_error()
                    })?;
                    let Some(dataframe_part) = response.data else {
                        continue;
                    };
                    let batch: RecordBatch = dataframe_part.try_into().map_err(|err| {
                        ApiError::deserialization_with_source(
                            trace_id,
                            err,
                            "decoding query_dataset response batch",
                        )
                        .into_df_error()
                    })?;

                    chunk_info_batches.push(batch);
                }
            }
            let chunk_info_batches = compute_unique_chunk_info_ids(chunk_info_batches)?;

            // Begin per-connection analytics tracking.
            // Fetch stats will be accumulated by the IO loops; the event is sent on drop.
            let pending_analytics = self.analytics.as_ref().map(|analytics| {
                let agg = chunk_info_batches
                    .as_ref()
                    .map(compute_chunk_info_aggregates)
                    .unwrap_or_default();

                analytics.begin_query(
                    crate::analytics::QueryInfo {
                        dataset_id: self.dataset_id.to_string(),
                        query_chunks: agg.chunks,
                        query_segments: agg.segments,
                        query_layers: agg.layers,
                        query_columns: self.schema.fields().len(),
                        query_entities: self.query_dataset_request.entity_paths.len(),
                        query_bytes: agg.bytes,
                        query_chunks_per_segment_max: agg.chunks_per_segment_max,
                        query_chunks_per_segment_mean: agg.chunks_per_segment_mean,
                        query_type: QueryType::classify(&self.query_expression),
                        primary_index_name: self
                            .query_expression
                            .filtered_index
                            .map(|i| i.as_str().to_owned()),
                        time_range: scan_start_wall..web_time::SystemTime::now(),
                        time_to_first_chunk_info,
                        trace_id,
                    },
                    scan_start,
                )
            });

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
                self.index_values.clone(),
                self.client.clone(),
                limit,
                #[cfg(not(target_arch = "wasm32"))]
                self.trace_headers.clone(),
                #[cfg(not(target_arch = "wasm32"))]
                trace_id,
                pending_analytics,
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
        .instrument(scan_span)
        .await
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

/// Extract entity paths referenced by the projected columns and filter expressions.
///
/// Returns `None` when no narrowing is possible (`projection` is `None`).
/// Returns `Some(empty set)` when projection contains only non-entity columns
/// (e.g. time / `segment_id`) — caller should not narrow in this case.
fn extract_projected_entity_paths(
    schema: &SchemaRef,
    projection: &Vec<usize>,
    filters: &[Expr],
) -> BTreeSet<EntityPath> {
    let mut entity_paths = BTreeSet::new();

    // Collect entity paths from projected columns.
    for &idx in projection {
        if let Some(path) = entity_path_from_field(schema.field(idx)) {
            entity_paths.insert(path);
        }
    }

    // Collect entity paths from filter-referenced columns. Filters may reference
    // columns that aren't in the projection (e.g. `WHERE t.b > 5` with only `t.a`
    // projected) — we must still fetch data for those entities.
    for filter in filters {
        for col_ref in filter.column_refs() {
            if let Ok(field) = schema.field_with_name(col_ref.name())
                && let Some(path) = entity_path_from_field(field)
            {
                entity_paths.insert(path);
            }
        }
    }

    entity_paths
}

/// Extract an [`EntityPath`] from an Arrow field's metadata, if present.
///
/// Component columns carry `rerun:entity_path` metadata; time/index columns
/// and the prepended `rerun_segment_id` column do not.
fn entity_path_from_field(field: &Field) -> Option<EntityPath> {
    field
        .metadata()
        .get(re_sorbet::metadata::SORBET_ENTITY_PATH)
        .map(|s| EntityPath::from(&**s))
}

/// Compute the output schema for a query on a dataset. When we call `get_dataset_schema`
/// on the Data Platform, we will get the schema for all entities and all components. This
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

#[tracing::instrument(level = "trace", skip_all)]
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
    chunk_info_batches: &[RecordBatch],
) -> Result<Arc<BTreeMap<String, Vec<RecordBatch>>>, DataFusionError> {
    let mut results: BTreeMap<String, Vec<RecordBatch>> = BTreeMap::new();

    for batch in chunk_info_batches {
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

            let segment_batch = re_arrow_util::take_record_batch(batch, &row_indices)?;

            results.entry(segment_id).or_default().push(segment_batch);
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
    pub chunks_per_segment_max: u32,
    pub chunks_per_segment_mean: f32,
}

pub(crate) fn compute_chunk_info_aggregates(batch: &RecordBatch) -> ChunkInfoAggregates {
    use arrow::array::UInt64Array;

    let chunks = batch.num_rows();

    /// Downcasts `column_name` to array type `T` and iterates over its non-null values.
    fn iter_column_values<'a, T: Any>(
        batch: &'a RecordBatch,
        column_name: &str,
    ) -> Option<std::iter::Flatten<<&'a T as IntoIterator>::IntoIter>>
    where
        &'a T: IntoIterator<Item: IntoIterator>,
    {
        let arr = batch
            .column_by_name(column_name)?
            .as_any()
            .downcast_ref::<T>()?;
        Some(arr.into_iter().flatten())
    }

    // Segment count + per-segment histogram in one pass
    let mut per_segment: HashMap<&str, u32> = HashMap::new();
    if let Some(items) =
        iter_column_values::<StringArray>(batch, QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID)
    {
        for v in items {
            *per_segment.entry(v).or_default() += 1;
        }
    }
    let segments = per_segment.len();
    let chunks_per_segment_max = per_segment.into_values().max().unwrap_or(0);
    let chunks_per_segment_mean = if segments == 0 {
        0.0
    } else {
        // chunks fits in u32 for realistic queries; precision loss is acceptable for analytics.
        chunks as f32 / segments as f32
    };

    let layers =
        iter_column_values::<StringArray>(batch, QueryDatasetResponse::FIELD_CHUNK_LAYER_NAME)
            .map(|iter| iter.collect::<HashSet<_>>().len())
            .unwrap_or(0);

    let bytes: u64 =
        iter_column_values::<UInt64Array>(batch, QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH)
            .map_or(0, Iterator::sum);

    ChunkInfoAggregates {
        chunks,
        segments,
        layers,
        bytes,
        chunks_per_segment_max,
        chunks_per_segment_mean,
    }
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
        columns_always_include_byte_offsets: true, // so we know exactly what to fetch from direct URLs
        columns_always_include_static_indexes: false,
        columns_always_include_global_indexes: false,
        columns_always_include_component_indexes: false,
    }
}

fn compute_unique_chunk_info_ids(
    chunk_info_batches: Vec<RecordBatch>,
) -> Result<Option<RecordBatch>, DataFusionError> {
    if chunk_info_batches.is_empty() {
        return Ok(None);
    }

    let schema = chunk_info_batches[0].schema();
    let combined = concat_batches(&schema, &chunk_info_batches)?;
    drop(chunk_info_batches);

    // Find the chunk_id column
    let chunk_id_col = combined
        .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
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

    Ok(Some(RecordBatch::try_new_with_options(
        schema,
        distinct_columns,
        &RecordBatchOptions::default(),
    )?))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, iter::once};

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
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
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
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_b1.len(), 1);
        assert_eq!(chunk_ids_b1.value(0), [1u8; 16]);
        let chunk_ids_b2 = group_b[1]
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_b2.len(), 1);
        assert_eq!(chunk_ids_b2.value(0), [4u8; 16]);

        let group_c = grouped.get("C").unwrap();
        assert_eq!(group_c.len(), 2);
        let chunk_ids_c1 = group_c[0]
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_c1.len(), 1);
        assert_eq!(chunk_ids_c1.value(0), [3u8; 16]);
        let chunk_ids_c2 = group_c[1]
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_c2.len(), 1);
        assert_eq!(chunk_ids_c2.value(0), [5u8; 16]);

        let group_d = grouped.get("D").unwrap();
        assert_eq!(group_d.len(), 1);
        let chunk_ids_d = group_d[0]
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        assert_eq!(chunk_ids_d.len(), 1);
        assert_eq!(chunk_ids_d.value(0), [6u8; 16]);
    }

    // ==================== Entity path projection pushdown tests ====================

    /// Build a schema mimicking `DataframeQueryTableProvider`'s output schema:
    /// - Index 0: `rerun_segment_id` (Utf8, no entity path metadata)
    /// - Index 1: `log_time` (Int64, with `rerun:kind=index` metadata)
    /// - Index 2: `/points:Position3D:positions` (component, `entity_path=/points`)
    /// - Index 3: `/points:Color:colors` (component, `entity_path=/points`)
    /// - Index 4: `/cameras:Transform3D:transform` (component, `entity_path=/cameras`)
    fn make_schema_with_entities() -> SchemaRef {
        use re_sorbet::metadata::{RERUN_KIND, SORBET_ENTITY_PATH};

        let index_metadata = HashMap::from([(RERUN_KIND.to_owned(), "index".to_owned())]);
        let points_metadata =
            HashMap::from([(SORBET_ENTITY_PATH.to_owned(), "/points".to_owned())]);
        let cameras_metadata =
            HashMap::from([(SORBET_ENTITY_PATH.to_owned(), "/cameras".to_owned())]);

        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("rerun_segment_id", DataType::Utf8, false),
                Field::new("log_time", DataType::Int64, false).with_metadata(index_metadata),
                Field::new("/points:Position3D:positions", DataType::Utf8, true)
                    .with_metadata(points_metadata.clone()),
                Field::new("/points:Color:colors", DataType::Utf8, true)
                    .with_metadata(points_metadata),
                Field::new("/cameras:Transform3D:transform", DataType::Utf8, true)
                    .with_metadata(cameras_metadata),
            ],
            HashMap::new(),
        ))
    }

    #[test]
    fn test_projection_single_entity() {
        let schema = make_schema_with_entities();
        // Select seg_id + log_time + both /points columns
        let projection = vec![0, 1, 2, 3];
        let paths = extract_projected_entity_paths(&schema, &projection, &[]);
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&EntityPath::from("/points")));
    }

    #[test]
    fn test_projection_multiple_entities() {
        let schema = make_schema_with_entities();
        // Select seg_id + one /points col + /cameras col
        let projection = vec![0, 2, 4];
        let paths = extract_projected_entity_paths(&schema, &projection, &[]);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&EntityPath::from("/points")));
        assert!(paths.contains(&EntityPath::from("/cameras")));
    }

    #[test]
    fn test_projection_only_non_entity_cols() {
        let schema = make_schema_with_entities();
        // Select only seg_id + log_time — no entity paths
        let projection = vec![0, 1];
        let paths = extract_projected_entity_paths(&schema, &projection, &[]);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_filter_adds_entity_paths() {
        use datafusion::logical_expr::col;

        let schema = make_schema_with_entities();
        // Project only /points column
        let projection = vec![0, 2];
        // Filter references /cameras column
        let filters = vec![col("/cameras:Transform3D:transform").is_not_null()];
        let paths = extract_projected_entity_paths(&schema, &projection, &filters);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&EntityPath::from("/points")));
        assert!(paths.contains(&EntityPath::from("/cameras")));
    }

    #[test]
    fn test_filter_with_non_entity_cols_only() {
        use datafusion::logical_expr::{col, lit};

        let schema = make_schema_with_entities();
        // Project only /points column
        let projection = vec![0, 2];
        // Filter references segment_id (no entity path) and time index (no entity path)
        let filters = vec![
            col("rerun_segment_id").eq(lit("seg_a")),
            col("log_time").gt(lit(100_i64)),
        ];
        let paths = extract_projected_entity_paths(&schema, &projection, &filters);
        // Only /points from projection — filters don't add entity paths
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&EntityPath::from("/points")));
    }

    #[test]
    fn test_narrowing_intersects_with_original() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![
                EntityPath::from("/points"),
                EntityPath::from("/cameras"),
                EntityPath::from("/meshes"),
            ],
            select_all_entity_paths: false,
            ..Default::default()
        };

        query
            .entity_paths
            .retain(|path| projected_paths.contains(path));

        assert_eq!(query.entity_paths, vec![EntityPath::from("/points")]);
    }

    #[test]
    fn test_narrowing_empty_projected_no_change() {
        let projected_paths: BTreeSet<EntityPath> = BTreeSet::new();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/cameras")],
            select_all_entity_paths: false,
            ..Default::default()
        };
        let original = query.entity_paths.clone();

        // Empty projected_paths → caller should skip narrowing
        if !projected_paths.is_empty() {
            query
                .entity_paths
                .retain(|path| projected_paths.contains(path));
        }

        assert_eq!(query.entity_paths, original);
    }

    #[test]
    fn test_narrowing_select_all_no_change() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![],
            select_all_entity_paths: true,
            ..Default::default()
        };

        // select_all_entity_paths=true → skip narrowing
        if !query.select_all_entity_paths && !query.entity_paths.is_empty() {
            query
                .entity_paths
                .retain(|path| projected_paths.contains(path));
        }

        assert!(query.entity_paths.is_empty());
        assert!(query.select_all_entity_paths);
    }

    #[test]
    fn test_narrowing_preserves_multiple_queries() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut queries = vec![
            QueryDatasetRequest {
                entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/cameras")],
                select_all_entity_paths: false,
                ..Default::default()
            },
            QueryDatasetRequest {
                entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/meshes")],
                select_all_entity_paths: false,
                ..Default::default()
            },
        ];

        for query in &mut queries {
            if !query.select_all_entity_paths && !query.entity_paths.is_empty() {
                query
                    .entity_paths
                    .retain(|path| projected_paths.contains(path));
            }
        }

        assert_eq!(queries[0].entity_paths, vec![EntityPath::from("/points")]);
        assert_eq!(queries[1].entity_paths, vec![EntityPath::from("/points")]);
    }

    #[test]
    fn test_narrowing_skipped_with_fill_latest_at() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/cameras")],
            select_all_entity_paths: false,
            ..Default::default()
        };
        let original = query.entity_paths.clone();

        // Simulate fill_latest_at=true check
        let sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;
        if sparse_fill_strategy == SparseFillStrategy::None && !projected_paths.is_empty() {
            query
                .entity_paths
                .retain(|path| projected_paths.contains(path));
        }

        assert_eq!(query.entity_paths, original);
    }
}
