use crate::analytics::{QueryInfo, QueryType, expr_filter_signature};
use crate::batch_coalescer::coalesce_exec::SizedCoalesceBatchesExec;
use crate::batch_coalescer::coalescer::CoalescerOptions;
use crate::pushdown_expressions::{apply_filter_expr_to_queries, filter_expr_is_supported};
use ahash::{HashMap, HashMapExt as _, HashSet};
use arrow::array::{
    ArrayRef, DurationNanosecondArray, Int64Array, RecordBatch, TimestampMicrosecondArray,
    TimestampMillisecondArray, TimestampNanosecondArray, TimestampSecondArray, UInt32Array,
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
use itertools::Itertools as _;
use parking_lot::Mutex;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::{Index, IndexValue, QueryExpression, SparseFillStrategy};
use re_log_types::{EntityPath, EntryId};
use re_protos::cloud::v1alpha1::ext::QueryDatasetDataframe;
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::cloud::v1alpha1::{
    FetchChunksRequest, GetDatasetSchemaRequest, GetDatasetSchemaResponse, QueryDatasetResponse,
};
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::{
    cloud::v1alpha1::ext::{Query, QueryDatasetRequest, QueryLatestAt, QueryRange},
    common::v1alpha1::ext::SegmentId,
};
use re_redap_client::{ApiError, ApiResult, ConnectionClient, ConnectionRegistryHandle};

use crate::{IntoDfError as _, SegmentStreamExec};
use re_sorbet::{
    BatchType, ChunkColumnDescriptors, ColumnDescriptor, ColumnKind, ComponentColumnSelector,
};
use re_uri::Origin;
use std::any::Any;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr as _;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::LazyLock;
use std::time::Duration;
use tracing::Instrument as _;
use web_time::{Instant, SystemTime};

/// Environment variable to force the client to go through the `FetchChunks` data fetching path.
#[cfg(not(target_arch = "wasm32"))]
static CHUNK_STRATEGY: LazyLock<String> = LazyLock::new(|| {
    std::env::var("RERUN_CHUNK_STRATEGY")
        .unwrap_or_default()
        .to_ascii_lowercase()
});

/// True when `RERUN_CHUNK_STRATEGY=grpc` — the client should fetch all chunks via
/// `FetchChunks` gRPC and the server should skip direct-URL generation.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn force_grpc() -> bool {
    *CHUNK_STRATEGY == "grpc"
}

/// On Wasm there are no environment variables, so gRPC is never forced via env.
#[cfg(target_arch = "wasm32")]
pub(crate) fn force_grpc() -> bool {
    false
}

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
pub(crate) type IndexValuesMap = Option<Arc<BTreeMap<SegmentId, BTreeSet<IndexValue>>>>;

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

    /// `query_metrics()` collectors that observed this provider at construction
    /// time. The list is captured once — typically by reading the Python
    /// `ContextVar` in `dataset_view.rs::reader()` — and travels with the
    /// provider into every `SegmentStreamExec` it builds. Empty when no
    /// `query_metrics()` scope was active.
    metrics_collectors: Vec<crate::MetricsCollector>,

    /// Filter expressions offered by DataFusion at planning time, stored for
    /// inclusion in the `cloud_query_dataset` analytics span.
    ///
    /// `Arc` so that the value survives any clone made between planning and scan.
    filter_capture: Arc<Mutex<Option<FilterCapture>>>,
}

/// Per-filter classification data captured in [`DataframeQueryTableProvider::supports_filters_pushdown`]
/// and consumed in [`DataframeQueryTableProvider::scan`].
#[derive(Default, Debug)]
struct FilterCapture {
    total: u32,

    /// Semicolon-delimited SQL signatures of all offered filters (in DataFusion's order).
    all_signatures: String,

    /// Signatures of filters classified as [`TableProviderFilterPushDown::Exact`].
    exact_signatures: String,

    /// Signatures of filters classified as [`TableProviderFilterPushDown::Inexact`].
    inexact_signatures: String,

    /// Signatures of filters classified as [`TableProviderFilterPushDown::Unsupported`].
    unsupported_signatures: String,
}

/// This trait provides the specific methods used when interacting with the
/// gRPC services for the datafusion client services.
///
/// By implementing this as a trait we can provide an alternative implementation
/// in our testing facility to remove all gRPC layers and test the server
/// responses more directly.
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
    pub async fn new(
        origin: Origin,
        connection_registry: ConnectionRegistryHandle,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
        segment_ids: &[impl AsRef<str> + Sync],
        index_values: IndexValuesMap,
        arrow_schema: Option<Schema>,
        #[cfg(not(target_arch = "wasm32"))] trace_headers: Option<crate::TraceHeaders>,
        metrics_collectors: Vec<crate::MetricsCollector>,
    ) -> ApiResult<Self> {
        let connection = connection_registry.connection(origin.clone()).await?;

        let mut provider = Self::new_from_client(
            connection.client,
            dataset_id,
            query_expression,
            segment_ids,
            index_values,
            arrow_schema,
            #[cfg(not(target_arch = "wasm32"))]
            trace_headers,
            metrics_collectors,
        )
        .await?;

        provider.analytics = connection.analytics.map(crate::ConnectionAnalytics::new);

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
        metrics_collectors: Vec<crate::MetricsCollector>,
    ) -> ApiResult<Self> {
        // Either use the caller-provided schema or fetch it from the server.
        let (schema, trace_id) = if let Some(schema) = arrow_schema {
            (schema, None)
        } else {
            let request = tonic::Request::new(GetDatasetSchemaRequest {}).with_entry_id(dataset_id);
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

        let entity_paths = query_expression
            .view_contents
            .as_ref()
            .map_or(vec![], |contents| {
                contents.keys().cloned().collect::<Vec<_>>()
            });

        // Preserve the `QueryExpression` distinction between:
        // - `view_contents=None`: all entities
        // - `view_contents=Some(empty)`: no entities
        //
        // Both cases produce an empty `entity_paths` list, so the explicit flag
        // must be driven from `view_contents` itself rather than the derived list.
        let select_all_entity_paths = query_expression.view_contents.is_none();

        let query = query_from_query_expression(
            query_expression,
            query_expression.sparse_fill_strategy != SparseFillStrategy::None,
        );
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
            // Skip server-side URL signing when the client is forced to fetch via gRPC —
            // signing would be wasted work and, on Azure, can fail outright with a 403
            // before the gRPC fetch path is ever reached.
            generate_direct_urls: !force_grpc(),
        };

        let schema = Arc::new(prepend_string_column_schema(
            &schema,
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
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
            metrics_collectors,
            filter_capture: Arc::new(Mutex::new(None)),
        })
    }

    /// Whether pushdown should synthesize a `latest_at` alongside its
    /// rewritten `range`. True iff the caller requested sparse-fill semantics
    /// — synthesizing `latest_at` under [`SparseFillStrategy::None`] would
    /// force an expensive server-side latest-at fan-out whose rows the
    /// caller has already opted out of.
    ///
    /// Same gate drives the entity-path projection narrowing block in `scan`
    /// (in its inverted form): both optimizations are only safe when no fill
    /// is requested.
    fn synthesize_latest_at(&self) -> bool {
        self.query_expression.sparse_fill_strategy != SparseFillStrategy::None
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
            let scan_start_wall = SystemTime::now();
            let scan_start = Instant::now();

            let synthesize_latest_at = self.synthesize_latest_at();
            let FilterCapture {
                total: filters_total,
                all_signatures: filters_signatures,
                exact_signatures: filters_signatures_exact,
                inexact_signatures: filters_signatures_inexact,
                unsupported_signatures: filters_signatures_unsupported,
            } = self.filter_capture.lock().take().unwrap_or_default();

            let mut dataset_queries = vec![self.query_dataset_request.clone()];
            let mut filters_pushed_down: usize = 0;
            let mut filters_applied_client_side: usize = 0;
            for filter in filters {
                match apply_filter_expr_to_queries(
                    dataset_queries.clone(),
                    filter,
                    &self.schema,
                    synthesize_latest_at,
                )? {
                    Some(updated_queries) => {
                        filters_pushed_down += 1;
                        dataset_queries = updated_queries;
                    }
                    None => {
                        filters_applied_client_side += 1;
                    }
                }
            }

            // Now that filter pushdown has settled each query's `segment_ids`, bound each query's chunk fetch
            // to its sampled index values.
            // Done here rather than at construction so the per-segment value lists stay aligned with `segment_ids` after a
            // `rerun_segment_id` predicate narrows or splits them.
            for query in &mut dataset_queries {
                apply_per_segment_pushdown(
                    query,
                    &self.index_values,
                    self.query_expression.filtered_index,
                );
            }

            // Entity path projection pushdown: narrow the server request to only
            // fetch chunks for entity paths that are actually needed by the projection
            // and filters. Skip when fill_latest_at is enabled, because timestamps
            // from excluded entities would produce rows with filled values that the
            // user expects.
            let mut entity_path_narrowing_applied = false;
            if !synthesize_latest_at
                && let Some(projected_paths) = projection.map(|projection| {
                    extract_projected_entity_paths(&self.schema, projection, filters)
                })
                && !projected_paths.is_empty()
            {
                for query in &mut dataset_queries {
                    if !query.select_all_entity_paths && !query.entity_paths.is_empty() {
                        let before = query.entity_paths.len();
                        query
                            .entity_paths
                            .retain(|path| projected_paths.contains(path));
                        if query.entity_paths.len() != before {
                            entity_path_narrowing_applied = true;
                        }
                    }
                }
            }

            // Component projection pushdown: narrow `fuzzy_descriptors` to the components the
            // projection and filters reference, so the server skips chunks for unselected ones
            // (e.g. a heavy `VideoStream:sample` next to a tiny `is_keyframe`). Gated on
            // `SparseFillStrategy::None`, like the entity-path narrowing above.
            //
            // Only narrow on a strict subset. An empty list means "all components" server-side,
            // and a full read projects every column, so listing all of them would be a no-op
            // were it not that the server treats a non-empty list as exhaustive and drops
            // static-only components (those with no temporal index). Leaving it empty is correct.
            //
            // TODO(RR-3157): ideally `DatasetView` would let users select components explicitly,
            // so we wouldn't have to infer them from the query.
            if self.query_expression.sparse_fill_strategy == SparseFillStrategy::None
                && let Some(projected_components) = projection.map(|projection| {
                    extract_projected_components(&self.schema, projection, filters)
                })
                && !projected_components.is_empty()
                && projected_components.len() < all_schema_components(&self.schema).len()
            {
                for query in &mut dataset_queries {
                    if query.fuzzy_descriptors.is_empty() {
                        query.fuzzy_descriptors = projected_components.iter().cloned().collect();
                    }
                }
            }

            let mut query_expression = self.query_expression.clone();

            let mut chunk_info_batches = Vec::with_capacity(dataset_queries.len());
            let mut time_to_first_chunk_info: Option<Duration> = None;

            let mut trace_id: Option<opentelemetry::TraceId> = None;

            for dataset_query in dataset_queries {
                let query_start = Instant::now();

                // Build the proto request once, then clone it per retry attempt (`tonic::Request`
                // isn't `Clone`). The server rejects `QueryDataset` with `ResourceExhausted`
                // fail-fast (before any work) when its stream-concurrency limiter is saturated, so
                // retrying the open is idempotent. Map tonic → `ApiError` *inside* the retry so the
                // predicate sees `ResourcesExhausted`, and to `DataFusionError` *outside*.
                let proto_request: re_protos::cloud::v1alpha1::QueryDatasetRequest =
                    dataset_query.into();
                let dataset_id = self.dataset_id;
                let response =
                    re_redap_client::with_retry_resource_exhausted("query_dataset", || {
                        let mut client = self.client.clone();
                        let request =
                            tonic::Request::new(proto_request.clone()).with_entry_id(dataset_id);
                        async move {
                            client
                                .query_dataset(request)
                                .await
                                .map_err(|err| ApiError::tonic(err, "query_dataset"))
                        }
                    })
                    .await
                    .map_err(|err| err.into_df_error())?;

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

            // Build the planning-phase summary unconditionally — it feeds both the
            // analytics span (when enabled) and the `MetricsSet` on the resulting
            // `SegmentStreamExec` (always).
            let agg = chunk_info_batches
                .as_ref()
                .map(compute_chunk_info_aggregates)
                .unwrap_or_default();
            let query_info = QueryInfo {
                dataset_id: self.dataset_id.to_string(),
                query_chunks: agg.chunks,
                query_segments: agg.segments,
                query_layers: agg.layers,
                query_columns: self.schema.fields().len(),
                query_entities: self.query_dataset_request.entity_paths.len(),
                query_bytes: agg.bytes,
                query_chunks_per_segment_min: agg.chunks_per_segment_min,
                query_chunks_per_segment_max: agg.chunks_per_segment_max,
                query_chunks_per_segment_mean: agg.chunks_per_segment_mean,
                query_type: QueryType::classify(&self.query_expression),
                primary_index_name: self
                    .query_expression
                    .filtered_index
                    .map(|i| i.as_str().to_owned()),
                time_to_first_chunk_info,
                trace_id,
                filters_pushed_down,
                filters_applied_client_side,
                entity_path_narrowing_applied,
                filters_total,
                filters_signatures,
                filters_signatures_exact,
                filters_signatures_inexact,
                filters_signatures_unsupported,
            };

            // Begin analytics tracking. This also constructs the plan's
            // `QueryMetrics` (fetch counters + ad-hoc `EXPLAIN ANALYZE`
            // MetricsSet), owned by the analytics struct as the single source
            // of truth — `SegmentStreamExec` reads it through
            // `PendingQueryAnalytics::metrics`. The PostHog OTLP send is gated
            // by `self.analytics.is_some()`; the resulting struct is always
            // returned so the `metrics_capture` subscribers and DataFusion
            // `metrics()` see the same data.
            let pending_analytics = crate::analytics::begin_query(
                self.analytics.clone(),
                query_info,
                scan_start,
                scan_start_wall,
            );

            // Find the first column selection that is a component
            if query_expression.filtered_is_not_null.is_none() {
                let filters = filters.iter().collect::<Vec<_>>();
                query_expression.filtered_is_not_null =
                    Self::compute_column_is_neq_null_filter(&filters)
                        .into_iter()
                        .flatten()
                        .next();
            }

            // `SegmentStreamExec` already emits batches sized by
            // `DEFAULT_BATCH_ROWS` / `DEFAULT_BATCH_BYTES` directly in
            // `dataframe_query_provider::send_next_row_batch`. We still wrap
            // it in `SizedCoalesceBatchesExec`: with the source-side sizing
            // the coalescer is mostly a pass-through, but it acts as a
            // physical-plan boundary that DataFusion's optimizer relies on
            // (removing it has been observed to confuse downstream sort /
            // projection nodes that reference `rerun_segment_id`).
            SegmentStreamExec::try_new(
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
                self.metrics_collectors.clone(),
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
        let synthesize_latest_at = self.synthesize_latest_at();
        let results: Vec<TableProviderFilterPushDown> = if let Some(col) = non_null_columns.first()
        {
            let col = *col;
            std::iter::zip(&filter_columns, filters)
                .map(|(column_selector, filter_expr)| {
                    if Some(col) == column_selector.as_ref() {
                        Ok(TableProviderFilterPushDown::Exact)
                    } else {
                        filter_expr_is_supported(
                            filter_expr,
                            &self.query_dataset_request,
                            &self.schema,
                            synthesize_latest_at,
                        )
                    }
                })
                .try_collect()?
        } else {
            filters
                .iter()
                .map(|filter_expr| {
                    filter_expr_is_supported(
                        filter_expr,
                        &self.query_dataset_request,
                        &self.schema,
                        synthesize_latest_at,
                    )
                })
                .try_collect()?
        };

        let mut all_sigs: Vec<String> = Vec::with_capacity(filters.len());
        let mut exact_sigs: Vec<String> = Vec::new();
        let mut inexact_sigs: Vec<String> = Vec::new();
        let mut unsupported_sigs: Vec<String> = Vec::new();
        for (filter, result) in std::iter::zip(filters, &results) {
            let sig = expr_filter_signature(filter);
            all_sigs.push(sig.clone());
            match result {
                TableProviderFilterPushDown::Exact => exact_sigs.push(sig),
                TableProviderFilterPushDown::Inexact => inexact_sigs.push(sig),
                TableProviderFilterPushDown::Unsupported => unsupported_sigs.push(sig),
            }
        }
        *self.filter_capture.lock() = Some(FilterCapture {
            total: filters.len() as u32,
            all_signatures: all_sigs.join(";"),
            exact_signatures: exact_sigs.join(";"),
            inexact_signatures: inexact_sigs.join(";"),
            unsupported_signatures: unsupported_sigs.join(";"),
        });

        Ok(results)
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

/// The component identifier of a field, or `None` if it isn't a component column.
///
/// Only genuine component columns carry an entity path; gating on it (the
/// same signal `extract_projected_entity_paths` uses) drops the prepended,
/// unmarked `rerun_segment_id`, which would otherwise be misclassified as a
/// `Component` (`ColumnKind` defaults to `Component` for unmarked fields).
/// Index/time columns also lack an entity path, so the same gate excludes
/// them (and as a backstop they carry `rerun:kind=index`, which
/// `try_from_arrow_field` maps to a non-`Component` descriptor).
fn component_from_field(field: &Field) -> Option<String> {
    field
        .metadata()
        .get(re_sorbet::metadata::SORBET_ENTITY_PATH)?;
    match ColumnDescriptor::try_from_arrow_field(None, field) {
        Ok(ColumnDescriptor::Component(component)) => Some(component.component.to_string()),
        _ => None,
    }
}

/// Every component identifier present in `schema`.
///
/// Used to detect a full projection: when the projection references every
/// component, narrowing `fuzzy_descriptors` is a no-op for chunk skipping, but
/// the server treats a non-empty list as exhaustive and would drop chunks for
/// static-only components (those with no temporal index). So we only narrow when
/// the projection is a strict subset.
fn all_schema_components(schema: &SchemaRef) -> BTreeSet<String> {
    schema
        .fields()
        .iter()
        .filter_map(|field| component_from_field(field))
        .collect()
}

/// Component identifiers referenced by a query's projection and filters.
///
/// Counterpart to [`extract_projected_entity_paths`] at component granularity:
/// it lets the scan narrow `fuzzy_descriptors` so the server skips chunks for
/// unselected components (e.g. a heavy `VideoStream:sample` sitting next to a tiny `is_keyframe`).
/// Time/index and `rerun_segment_id` columns are not components and are simply ignored.
fn extract_projected_components(
    schema: &SchemaRef,
    projection: &[usize],
    filters: &[Expr],
) -> BTreeSet<String> {
    let mut components = BTreeSet::new();

    for &idx in projection {
        if let Some(component) = component_from_field(schema.field(idx)) {
            components.insert(component);
        }
    }

    // Filters may reference components outside the projection (e.g. `WHERE
    // is_keyframe IS NOT NULL` while only the index is projected); those chunks
    // are still needed to evaluate the filter, so keep them too.
    for filter in filters {
        for col_ref in filter.column_refs() {
            if let Ok(field) = schema.field_with_name(col_ref.name())
                && let Some(component) = component_from_field(field)
            {
                components.insert(component);
            }
        }
    }

    components
}

/// Compute the output schema for a query on a dataset. When we call `get_dataset_schema`
/// on the catalog server, we will get the schema for all entities and all components. This
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

pub(crate) fn prepend_string_column_schema(schema: &Schema, column_name: &str) -> Schema {
    let mut fields = vec![Field::new(column_name, DataType::Utf8, false)];
    fields.extend(schema.fields().iter().map(|f| (**f).clone()));
    Schema::new_with_metadata(fields, schema.metadata.clone())
}

/// Hash a segment id for DataFusion partition routing.
///
/// Hashes the underlying string with DataFusion's `HashValue` so the result
/// matches `RepartitionExec`'s hashing of the segment-id string column.
pub(crate) fn segment_partition_hash(
    segment_id: &SegmentId,
    random_state: &ahash::RandomState,
) -> u64 {
    use datafusion::common::hash_utils::HashValue as _;
    segment_id.as_str().hash_one(random_state)
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

/// Bound a single settled query's server-side chunk scan to the chunks covering
/// its sampled index values, by setting them as `QueryLatestAt.per_segment_values`
/// (aligned to the query's own `segment_ids`).
///
/// Runs in [`DataframeQueryTableProvider::scan`] *after* filter pushdown has
/// narrowed or split `segment_ids`, so the per-segment value lists always line up
/// with the segment ids the server validates against. Without it the server
/// fetches every segment's entire chunk set; it narrows only what is fetched, not
/// the rows produced.
fn apply_per_segment_pushdown(
    query_request: &mut QueryDatasetRequest,
    index_values: &IndexValuesMap,
    timeline: Option<Index>,
) {
    let Some(index_values) = index_values.as_ref() else {
        return; // no `using_index_values`, nothing to push
    };
    let Some(timeline) = timeline else {
        return; // static-only query, no index to bound
    };
    // The contract requires non-empty, positionally-matched segment ids. An
    // unscoped reader (no `filter_segments`) sends none, so there is nothing to
    // align against.
    if query_request.segment_ids.is_empty() {
        return;
    }
    let per_segment_values = per_segment_values_aligned(&query_request.segment_ids, index_values);

    let Some(query) = query_request.query.as_mut() else {
        return;
    };
    query.latest_at = Some(QueryLatestAt {
        index: Some(timeline),
        at: IndexValue::STATIC,
        per_segment_values,
    });
    query.range = None;
}

/// Build `QueryLatestAt.per_segment_values` for `segment_ids`, looking each up in `index_values`.
///
/// One list per segment, in order, since the server matches them positionally against `QueryDatasetRequest.segment_ids`.
/// Segments absent from the map get an empty list and `STATIC` values are skipped.
fn per_segment_values_aligned(
    segment_ids: &[SegmentId],
    index_values: &BTreeMap<SegmentId, BTreeSet<IndexValue>>,
) -> Vec<Vec<i64>> {
    segment_ids
        .iter()
        .map(|segment_id| {
            index_values
                .get(segment_id)
                .map(|values| {
                    values
                        .iter()
                        .filter(|value| !value.is_static())
                        .map(|value| value.as_i64())
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

/// Build a server-side [`Query`] from a [`QueryExpression`].
///
/// `synthesize_latest_at` controls whether a non-static query carries a
/// `latest_at` derived from [`QueryExpression::min_latest_at`]. Pass `true`
/// when the caller wants sparse-fill semantics
/// ([`SparseFillStrategy::LatestAtGlobal`]); pass `false` (the common case,
/// [`SparseFillStrategy::None`]) to skip — without fill, the `range` already
/// carries everything the server needs and the synthesized `latest_at` would
/// only drive an expensive latest-at fan-out for rows the caller has opted
/// out of.
///
/// Static-only queries ([`QueryExpression::is_static`]) always carry
/// [`QueryLatestAt::new_static`] regardless of the flag — that marker is the
/// sole signal the server uses to filter to static chunks; stripping it
/// would turn a static-only request into a full-dataset scan.
pub fn query_from_query_expression(
    query_expression: &QueryExpression,
    synthesize_latest_at: bool,
) -> Query {
    let latest_at = if query_expression.is_static() {
        Some(QueryLatestAt::new_static())
    } else if synthesize_latest_at {
        query_expression
            .min_latest_at()
            .map(|latest_at| QueryLatestAt::global(latest_at.timeline(), latest_at.at()))
    } else {
        None
    };

    Query {
        latest_at,
        range: query_expression.max_range().map(|range| QueryRange {
            index: *range.timeline(),
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

    let chunk_ids = QueryDatasetDataframe::COLUMN_CHUNK_ID
        .extract(&combined)
        .map_err(|err| exec_datafusion_err!("{err}"))?;

    let mut indices_to_keep = Vec::new();
    let mut seen: HashSet<[u8; 16]> = HashSet::default();

    for (row_idx, chunk_id) in chunk_ids.iter().enumerate() {
        if seen.insert(*chunk_id) {
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

    use arrow::array::{FixedSizeBinaryBuilder, StringArray};
    use re_protos::cloud::v1alpha1::ext;

    use super::*;

    #[test]
    fn per_segment_values_align_to_segment_ids_order() {
        let seg = |s: &str| SegmentId::from(s);
        let at = IndexValue::new_temporal;

        let index_values: BTreeMap<SegmentId, BTreeSet<IndexValue>> = [
            (seg("a"), BTreeSet::from([at(30), at(10), at(20)])),
            (seg("b"), BTreeSet::from([at(5)])),
        ]
        .into_iter()
        .collect();

        // Output is positionally matched to `segment_ids`
        // "b" comes first here, and "c" (absent from the map) yields an empty list rather than being skipped.
        let segment_ids = [seg("b"), seg("a"), seg("c")];
        let values = per_segment_values_aligned(&segment_ids, &index_values);

        assert_eq!(values, vec![vec![5], vec![10, 20, 30], vec![]]);
    }

    #[test]
    fn per_segment_values_skip_static() {
        let index_values: BTreeMap<SegmentId, BTreeSet<IndexValue>> = std::iter::once((
            SegmentId::from("a"),
            BTreeSet::from([IndexValue::STATIC, IndexValue::new_temporal(7)]),
        ))
        .collect();

        let values = per_segment_values_aligned(&[SegmentId::from("a")], &index_values);

        assert_eq!(values, vec![vec![7]]);
    }

    /// Per-segment pushdown stays aligned with `segment_ids` when a
    /// `rerun_segment_id` predicate narrows them, and rewrites the survivor into
    /// the bounded encoding: sorted `per_segment_values`, `range` dropped, and `at`
    /// parked at STATIC against the query's index.
    #[test]
    fn segment_id_filter_keeps_per_segment_values_aligned() {
        use datafusion::logical_expr::{col, lit};

        let seg = |s: &str| SegmentId::from(s);
        let at = IndexValue::new_temporal;
        let timeline = Index::from("my_index");

        // Three scoped segments, each sampled at specific index values, as set by
        // `filter_segments([…]).reader(using_index_values={…})`.
        let segment_ids = vec![seg("a"), seg("b"), seg("c")];
        // "b" carries two values so the survivor exercises sorted multi-value emission.
        let map: BTreeMap<SegmentId, BTreeSet<IndexValue>> = [
            (seg("a"), BTreeSet::from([at(10), at(20)])),
            (seg("b"), BTreeSet::from([at(15), at(5)])),
            (seg("c"), BTreeSet::from([at(7), at(8)])),
        ]
        .into_iter()
        .collect();
        let index_values: IndexValuesMap = Some(Arc::new(map));

        // Baseline request as built before any pushdown: a temporal `range`, and
        // no `per_segment_values` yet (the pushdown is deferred to `scan`).
        let request = QueryDatasetRequest {
            segment_ids: segment_ids.clone(),
            query: Some(Query {
                latest_at: None,
                range: Some(QueryRange {
                    index: timeline,
                    index_range: re_log_types::AbsoluteTimeRange::new(at(0), at(100)),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        // 1. A `WHERE rerun_segment_id = 'b'` predicate is pushed down, narrowing
        //    `segment_ids` exactly as `scan`'s filter loop does. (The index field's
        //    `rerun:kind=index` metadata is irrelevant to a segment-id predicate,
        //    but mirrors a real query schema.)
        let schema = {
            let mut index_meta = HashMap::new();
            index_meta.insert(
                re_sorbet::metadata::RERUN_KIND.to_owned(),
                "index".to_owned(),
            );
            Arc::new(Schema::new_with_metadata(
                vec![
                    Field::new("my_index", DataType::Int64, false).with_metadata(index_meta),
                    Field::new("rerun_segment_id", DataType::Utf8, false),
                ],
                HashMap::default(),
            ))
        };
        let expr = col("rerun_segment_id").eq(lit("b"));
        let mut narrowed = apply_filter_expr_to_queries(vec![request], &expr, &schema, false)
            .unwrap()
            .expect("segment-id predicate is pushed down");
        assert_eq!(narrowed.len(), 1);

        // 2. Per-segment pushdown runs after narrowing and aligns to the survivor.
        apply_per_segment_pushdown(&mut narrowed[0], &index_values, Some(timeline));

        let narrowed = &narrowed[0];
        assert_eq!(narrowed.segment_ids, vec![seg("b")]);

        // `per_segment_values` is aligned to the surviving segment "b" → `[[5, 15]]`
        // (sorted), not the stale full `[[10, 20], [5, 15], [7, 8]]`. `range` is
        // cleared and `at` parked at STATIC against the query's index — the encoding
        // required alongside `per_segment_values`.
        let query = narrowed.query.as_ref().unwrap();
        let la = query.latest_at.as_ref().unwrap();
        assert_eq!(
            la.per_segment_values,
            vec![vec![5, 15]],
            "per_segment_values must align to the surviving segment_ids, sorted"
        );
        assert_eq!(la.index, Some(timeline));
        assert_eq!(la.at, IndexValue::STATIC);
        assert!(query.range.is_none());

        let wire: re_protos::cloud::v1alpha1::QueryDatasetRequest = narrowed.clone().into();
        re_protos::cloud::v1alpha1::ext::QueryDatasetRequest::try_from(wire)
            .expect("server must accept the request after segment-id narrowing");
    }

    /// `apply_per_segment_pushdown` only fires for a scoped temporal query with
    /// sampled values. Every other case leaves the request untouched, so the
    /// server falls back to its normal full per-segment scan.
    #[test]
    fn pushdown_is_noop_unless_scoped_temporal() {
        let seg = |s: &str| SegmentId::from(s);
        let timeline = Index::from("my_index");
        let values: IndexValuesMap = Some(Arc::new(
            once((seg("a"), BTreeSet::from([IndexValue::new_temporal(10)]))).collect(),
        ));

        let baseline = || QueryDatasetRequest {
            segment_ids: vec![seg("a")],
            query: Some(Query {
                latest_at: None,
                range: Some(QueryRange {
                    index: timeline,
                    index_range: re_log_types::AbsoluteTimeRange::new(
                        IndexValue::new_temporal(0),
                        IndexValue::new_temporal(100),
                    ),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let assert_untouched = |request: &QueryDatasetRequest| {
            let query = request.query.as_ref().unwrap();
            assert!(query.range.is_some(), "range must survive a no-op");
            assert!(
                query.latest_at.is_none(),
                "no `per_segment_values` must be pushed on a no-op"
            );
        };

        // No `using_index_values`.
        let no_values: IndexValuesMap = None;
        let mut req = baseline();
        apply_per_segment_pushdown(&mut req, &no_values, Some(timeline));
        assert_untouched(&req);

        // Static-only query: no index to bound against.
        let mut req = baseline();
        apply_per_segment_pushdown(&mut req, &values, None);
        assert_untouched(&req);

        // Unscoped reader: no `segment_ids` to align values to.
        let mut req = baseline();
        req.segment_ids.clear();
        apply_per_segment_pushdown(&mut req, &values, Some(timeline));
        assert_untouched(&req);
    }

    #[test]
    fn test_batches_grouping() {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                Arc::new(ext::QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field()),
                Arc::new(ext::QueryDatasetDataframe::COLUMN_CHUNK_ID.arrow_field()),
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

    /// Like [`make_schema_with_entities`] but with `rerun:component` metadata, so
    /// component columns parse to their real identifiers (`positions`, `colors`,
    /// `transform`) instead of falling back to the full column name — matching
    /// the metadata that real dataset schemas carry.
    fn make_schema_with_components() -> SchemaRef {
        use re_sorbet::metadata::{RERUN_KIND, SORBET_ENTITY_PATH};
        // `re_types_core::FIELD_METADATA_KEY_COMPONENT`.
        const COMPONENT: &str = "rerun:component";

        let index_metadata = HashMap::from([(RERUN_KIND.to_owned(), "index".to_owned())]);
        let component_metadata = |entity: &str, component: &str| {
            HashMap::from([
                (SORBET_ENTITY_PATH.to_owned(), entity.to_owned()),
                (COMPONENT.to_owned(), component.to_owned()),
            ])
        };

        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("rerun_segment_id", DataType::Utf8, false),
                Field::new("log_time", DataType::Int64, false).with_metadata(index_metadata),
                Field::new("/points:Position3D:positions", DataType::Utf8, true)
                    .with_metadata(component_metadata("/points", "positions")),
                Field::new("/points:Color:colors", DataType::Utf8, true)
                    .with_metadata(component_metadata("/points", "colors")),
                Field::new("/cameras:Transform3D:transform", DataType::Utf8, true)
                    .with_metadata(component_metadata("/cameras", "transform")),
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
    fn test_component_projection_single() {
        let schema = make_schema_with_components();
        // Select seg_id + log_time + only the positions component of /points.
        let projection = vec![0, 1, 2];
        let components = extract_projected_components(&schema, &projection, &[]);
        assert_eq!(
            components,
            once("positions".to_owned()).collect::<BTreeSet<_>>(),
            "only the projected component should be selected, not its sibling `colors`",
        );
    }

    #[test]
    fn test_component_projection_skips_non_component_columns() {
        let schema = make_schema_with_components();
        // Select only seg_id + log_time — neither is a component column.
        let projection = vec![0, 1];
        let components = extract_projected_components(&schema, &projection, &[]);
        assert!(
            components.is_empty(),
            "segment-id and index columns must not be treated as components",
        );
    }

    #[test]
    fn test_component_projection_filter_adds_component() {
        use datafusion::logical_expr::col;

        let schema = make_schema_with_components();
        // Project only the positions component, but filter on a sibling component.
        let projection = vec![0, 2];
        let filters = vec![col("/points:Color:colors").is_not_null()];
        let components = extract_projected_components(&schema, &projection, &filters);
        assert_eq!(
            components,
            ["positions".to_owned(), "colors".to_owned()]
                .into_iter()
                .collect::<BTreeSet<_>>(),
            "a component referenced only by a filter must still be fetched",
        );
    }

    #[test]
    fn test_all_schema_components() {
        let schema = make_schema_with_components();
        assert_eq!(
            all_schema_components(&schema),
            [
                "positions".to_owned(),
                "colors".to_owned(),
                "transform".to_owned()
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            "every component column in the schema must be reported, deduped",
        );
    }

    #[test]
    fn test_component_projection_full_read_is_not_narrowed() {
        // A full read projects every column. The projected components then equal
        // the full schema set, so the scan must NOT narrow `fuzzy_descriptors`
        // (an exhaustive list would drop static-only components server-side).
        let schema = make_schema_with_components();
        let projection: Vec<usize> = (0..schema.fields().len()).collect();
        let projected = extract_projected_components(&schema, &projection, &[]);
        assert_eq!(
            projected,
            all_schema_components(&schema),
            "projecting all columns must reference every component",
        );
        assert!(
            projected.len() >= all_schema_components(&schema).len(),
            "a full projection is not a strict subset, so narrowing must be skipped",
        );
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

    // -------------------------------------------------------------------
    // `query_from_query_expression` — gating of `latest_at` synthesis on
    // `synthesize_latest_at` for the user-supplied-range path. This is the
    // path that previously had NO coverage: a `QueryExpression` carrying a
    // `filtered_index_range` (or `filtered_index_values` / `using_index_values`)
    // with `sparse_fill_strategy = None`. Pre-altitude-fix, this path would
    // ship `latest_at=Some(...)` to the server because `min_latest_at()`
    // populated it unconditionally — the scrub helper was the safety net.
    // Post-fix, `synthesize_latest_at=false` must drop the latest_at at
    // construction; the static-only marker must survive regardless.
    // -------------------------------------------------------------------

    #[test]
    fn test_query_from_expr_user_supplied_range_no_fill_drops_latest_at() {
        use re_log_types::{AbsoluteTimeRange, TimeInt};

        let query_expression = QueryExpression {
            view_contents: None,
            filtered_index: Some("frame_nr".into()),
            filtered_index_range: Some(AbsoluteTimeRange::new(
                TimeInt::new_temporal(100),
                TimeInt::new_temporal(200),
            )),
            sparse_fill_strategy: SparseFillStrategy::None,
            ..Default::default()
        };

        let query = super::query_from_query_expression(&query_expression, false);

        assert!(
            query.latest_at.is_none(),
            "user-supplied range with sparse_fill=None must not ship latest_at",
        );
        let range = query.range.as_ref().expect("range must be set");
        assert_eq!(
            range.index_range,
            AbsoluteTimeRange {
                min: TimeInt::new_temporal(100),
                max: TimeInt::new_temporal(200),
            },
            "range must come from filtered_index_range verbatim",
        );
    }

    #[test]
    fn test_query_from_expr_user_supplied_range_fill_keeps_latest_at() {
        use re_log_types::{AbsoluteTimeRange, TimeInt};

        let query_expression = QueryExpression {
            view_contents: None,
            filtered_index: Some("frame_nr".into()),
            filtered_index_range: Some(AbsoluteTimeRange::new(
                TimeInt::new_temporal(100),
                TimeInt::new_temporal(200),
            )),
            sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
            ..Default::default()
        };

        let query = super::query_from_query_expression(&query_expression, true);

        let la = query
            .latest_at
            .as_ref()
            .expect("LatestAtGlobal must ship latest_at");
        assert_eq!(la.at, TimeInt::new_temporal(100));
        assert!(
            la.index.is_some(),
            "non-static latest_at must carry timeline"
        );
        assert!(query.range.is_some(), "range must still be set");
    }

    #[test]
    fn test_query_from_expr_static_preserves_new_static_regardless_of_flag() {
        // `filtered_index = None` ⇒ `is_static() = true` ⇒ `latest_at` must
        // be `new_static()` even when `synthesize_latest_at=false`. The
        // static marker is the sole signal the server uses to filter to
        // static chunks; stripping it turns a static-only request into a
        // full-dataset scan.
        let query_expression = QueryExpression {
            view_contents: None,
            filtered_index: None,
            sparse_fill_strategy: SparseFillStrategy::None,
            ..Default::default()
        };

        let query = super::query_from_query_expression(&query_expression, false);

        let la = query
            .latest_at
            .as_ref()
            .expect("static-only must carry new_static() latest_at");
        assert!(
            la.is_static(),
            "latest_at must be `new_static()` for static-only queries even when \
             synthesize_latest_at=false",
        );
        assert!(query.range.is_none(), "static-only must have no range");
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
                Arc::new(ext::QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field()),
                Arc::new(ext::QueryDatasetDataframe::COLUMN_RERUN_SEGMENT_LAYER.arrow_field()),
                Arc::new(ext::QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN.arrow_field()),
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
