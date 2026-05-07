use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;

use crate::IntoDfError as _;
use crate::PendingTableQueryAnalytics;
use crate::analytics::QueryErrorKind;
use crate::batch_coalescer::coalesce_exec::SizedCoalesceBatchesExec;
use crate::batch_coalescer::coalescer::CoalescerOptions;
use arrow::array::{Array as _, RecordBatch};
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::not_impl_err;
use datafusion::error::{DataFusionError, Result as DataFusionResult};
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::logical_expr::TableProviderFilterPushDown;
use datafusion::logical_expr::dml::InsertOp;
use datafusion::physical_plan::ExecutionPlan;
use datafusion::physical_plan::streaming::{PartitionStream, StreamingTableExec};
use datafusion::prelude::Expr;
use futures_util::StreamExt as _;
use re_redap_client::{ApiResponseStream, ApiResult};
use tokio_stream::Stream;

#[async_trait]
pub trait GrpcStreamToTable:
    std::fmt::Debug + 'static + Send + Sync + Clone + std::marker::Unpin
{
    type GrpcStreamData;

    async fn fetch_schema(&mut self) -> ApiResult<SchemaRef>;

    fn process_response(&mut self, response: Self::GrpcStreamData) -> ApiResult<RecordBatch>;

    async fn send_streaming_request(
        &mut self,
    ) -> ApiResult<ApiResponseStream<Self::GrpcStreamData>>;

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> DataFusionResult<Vec<TableProviderFilterPushDown>> {
        Ok(vec![
            TableProviderFilterPushDown::Unsupported;
            filters.len()
        ])
    }

    async fn insert_into(
        &self,
        _state: &dyn Session,
        _input: Arc<dyn ExecutionPlan>,
        _insert_op: InsertOp,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        not_impl_err!("Insert into not implemented for this table")
    }

    /// Optional analytics hook called once per `scan()`. Implementors that
    /// represent user-visible table scans (currently only
    /// `TableEntryTableProvider`) return a tracker that accumulates per-batch
    /// stats and emits an OTLP span on drop.
    fn begin_scan_analytics(
        &self,
        _schema: &SchemaRef,
        _projection: Option<&Vec<usize>>,
        _limit: Option<usize>,
    ) -> Option<PendingTableQueryAnalytics> {
        None
    }
}

#[derive(Debug)]
pub struct GrpcStreamProvider<T: GrpcStreamToTable> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcStreamToTable> GrpcStreamProvider<T> {
    pub async fn prepare(mut client: T) -> Result<Arc<Self>, DataFusionError> {
        let schema = client
            .fetch_schema()
            .await
            .map_err(|err| err.into_df_error())?;
        Ok(Arc::new(Self { schema, client }))
    }
}

#[async_trait]
impl<T> TableProvider for GrpcStreamProvider<T>
where
    T: GrpcStreamToTable + Send + 'static,
    T::GrpcStreamData: Send + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> datafusion::datasource::TableType {
        datafusion::datasource::TableType::Base
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        limit: Option<usize>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        let analytics = self
            .client
            .begin_scan_analytics(&self.schema, projection, limit);

        StreamingTableExec::try_new(
            self.schema.clone(),
            vec![Arc::new(GrpcStreamPartitionStream::new(
                &self.schema,
                self.client.clone(),
                analytics,
            ))],
            projection,
            Vec::default(),
            false,
            None,
        )
        .map(|e| Arc::new(e) as Arc<dyn ExecutionPlan>)
        .map(|exec| {
            Arc::new(SizedCoalesceBatchesExec::new(
                exec,
                CoalescerOptions {
                    target_batch_rows: crate::dataframe_query_common::DEFAULT_BATCH_ROWS,
                    target_batch_bytes: crate::dataframe_query_common::DEFAULT_BATCH_BYTES,
                    max_rows: limit,
                },
            )) as Arc<dyn ExecutionPlan>
        })
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> DataFusionResult<Vec<TableProviderFilterPushDown>> {
        self.client.supports_filters_pushdown(filters)
    }

    async fn insert_into(
        &self,
        state: &dyn Session,
        input: Arc<dyn ExecutionPlan>,
        insert_op: InsertOp,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        self.client.insert_into(state, input, insert_op).await
    }
}

#[derive(Debug)]
pub struct GrpcStreamPartitionStream<T: GrpcStreamToTable> {
    schema: SchemaRef,
    client: T,
    analytics: Option<PendingTableQueryAnalytics>,
}

impl<T: GrpcStreamToTable> GrpcStreamPartitionStream<T> {
    fn new(schema: &SchemaRef, client: T, analytics: Option<PendingTableQueryAnalytics>) -> Self {
        Self {
            schema: Arc::clone(schema),
            client,
            analytics,
        }
    }
}

impl<T> PartitionStream for GrpcStreamPartitionStream<T>
where
    T: GrpcStreamToTable + Send + 'static,
    T::GrpcStreamData: Send + 'static,
{
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        Box::pin(GrpcStream::execute(
            &self.schema,
            self.client.clone(),
            self.analytics.clone(),
        ))
    }
}

pub struct GrpcStream {
    schema: SchemaRef,
    adapted_stream: Pin<Box<dyn Stream<Item = datafusion::common::Result<RecordBatch>> + Send>>,
}

impl GrpcStream {
    fn execute<T>(
        schema: &SchemaRef,
        mut client: T,
        analytics: Option<PendingTableQueryAnalytics>,
    ) -> Self
    where
        T::GrpcStreamData: Send + 'static,
        T: GrpcStreamToTable + Send + 'static,
    {
        let adapted_stream = Box::pin(async_stream::try_stream! {
            let mut stream = client.send_streaming_request().await
                .map_err(|err| {
                    if let Some(analytics) = analytics.as_ref() {
                        analytics.record_error(QueryErrorKind::GrpcFetch);
                    }
                    err.into_df_error()
                })?;

            let trace_id = stream.trace_id();
            if let (Some(analytics), Some(trace_id)) = (analytics.as_ref(), trace_id) {
                analytics.record_trace_id(trace_id);
            }

            while let Some(msg) = stream.next().await {
                let msg = msg.map_err(|err| {
                        if let Some(analytics) = analytics.as_ref() {
                            analytics.record_error(QueryErrorKind::GrpcFetch);
                        }
                        err.into_df_error()
                    })?;
                if let Some(analytics) = analytics.as_ref() {
                    analytics.record_first_response();
                }
                let processed = client.process_response(msg)
                    .map_err(|err| {
                        if let Some(analytics) = analytics.as_ref() {
                            analytics.record_error(QueryErrorKind::Decode);
                        }
                        err.with_trace_id(trace_id).into_df_error()
                    })?;
                if let Some(analytics) = analytics.as_ref() {
                    analytics.record_first_batch();
                    let num_rows = processed.num_rows() as u64;
                    let num_bytes: u64 = processed
                        .columns()
                        .iter()
                        .map(|c| c.get_array_memory_size() as u64)
                        .sum();
                    analytics.record_batch(num_rows, num_bytes);
                }
                yield processed;
            }
        });

        Self {
            schema: Arc::clone(schema),
            adapted_stream,
        }
    }
}

impl RecordBatchStream for GrpcStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

impl Stream for GrpcStream {
    type Item = DataFusionResult<RecordBatch>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.adapted_stream.poll_next_unpin(cx)
    }
}

#[cfg(test)]
mod table_query_pipeline_tests {
    //! End-to-end tests that drive [`GrpcStream::execute`] with a deterministic
    //! fake [`GrpcStreamToTable`] impl and assert on the analytics state
    //! recorded into the resulting OTLP span.

    use std::collections::HashMap;
    use std::sync::Arc;

    use arrow::array::{RecordBatchOptions, UInt32Array};
    use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
    use async_trait::async_trait;
    use futures_util::StreamExt as _;
    use parking_lot::Mutex;
    use re_redap_client::{ApiError, ApiResponseStream, ApiResult};
    use re_uri::Origin;

    use crate::analytics::{TableQueryInfo, build_table_query_span};
    use crate::{ConnectionAnalytics, PendingTableQueryAnalytics, TableKind, TableQueryCaller};

    use super::*;

    /// Test-only [`GrpcStreamToTable`] impl with deterministic, configurable
    /// behavior. Owns a queue of items the stream will yield, plus knobs to
    /// fail at each stage of the pipeline.
    #[derive(Debug, Clone)]
    struct FakeProvider {
        /// Items the stream yields, consumed on the first `send_streaming_request` call.
        /// `Ok(v)` ⇒ yields a message that decodes to a single-row batch with `v`.
        /// `Err(_)` ⇒ yields a stream-level error (simulates a mid-stream gRPC failure).
        items: Arc<Mutex<Option<Vec<ApiResult<u32>>>>>,
        fail_send_request: bool,
        fail_decode: bool,
        trace_id: Option<opentelemetry::TraceId>,
    }

    impl FakeProvider {
        fn new(items: Vec<ApiResult<u32>>) -> Self {
            Self {
                items: Arc::new(Mutex::new(Some(items))),
                fail_send_request: false,
                fail_decode: false,
                trace_id: None,
            }
        }

        fn with_trace_id(mut self, trace_id: opentelemetry::TraceId) -> Self {
            self.trace_id = Some(trace_id);
            self
        }

        fn fail_send_request() -> Self {
            Self {
                items: Arc::new(Mutex::new(Some(vec![]))),
                fail_send_request: true,
                fail_decode: false,
                trace_id: None,
            }
        }

        fn fail_decode(items: Vec<u32>) -> Self {
            Self {
                items: Arc::new(Mutex::new(Some(
                    items.into_iter().map(Ok).collect::<Vec<_>>(),
                ))),
                fail_send_request: false,
                fail_decode: true,
                trace_id: None,
            }
        }
    }

    fn fake_schema() -> SchemaRef {
        Arc::new(Schema::new_with_metadata(
            vec![Field::new("v", DataType::UInt32, false)],
            HashMap::default(),
        ))
    }

    #[async_trait]
    impl GrpcStreamToTable for FakeProvider {
        type GrpcStreamData = u32;

        async fn fetch_schema(&mut self) -> ApiResult<SchemaRef> {
            Ok(fake_schema())
        }

        fn process_response(&mut self, response: u32) -> ApiResult<RecordBatch> {
            if self.fail_decode {
                return Err(ApiError::deserialization(None, "fake decode error"));
            }
            let arr = UInt32Array::from(vec![response]);
            RecordBatch::try_new_with_options(
                fake_schema(),
                vec![Arc::new(arr)],
                &RecordBatchOptions::new().with_row_count(Some(1)),
            )
            .map_err(|err| ApiError::internal_with_source(None, err, "build batch"))
        }

        async fn send_streaming_request(
            &mut self,
        ) -> ApiResult<ApiResponseStream<Self::GrpcStreamData>> {
            if self.fail_send_request {
                return Err(ApiError::deserialization(None, "fake send error"));
            }
            let items = self.items.lock().take().unwrap_or_default();
            let stream = futures_util::stream::iter(items);
            Ok(ApiResponseStream::new(stream, self.trace_id))
        }
    }

    fn make_pending() -> PendingTableQueryAnalytics {
        let origin: Origin = "rerun+http://localhost:51234".parse().unwrap();
        let analytics = ConnectionAnalytics::new(origin);
        analytics.begin_table_query(
            TableQueryInfo {
                table_id: "tbl-pipeline".to_owned(),
                table_kind: TableKind::Lance,
                caller: TableQueryCaller::CatalogResolver,
                schema_total_columns: 1,
                projected_columns: 1,
                has_limit: false,
                limit_value: None,
                time_range: web_time::SystemTime::now()..web_time::SystemTime::now(),
            },
            web_time::Instant::now(),
        )
    }

    fn find_int(span: &opentelemetry_proto::tonic::trace::v1::Span, key: &str) -> Option<i64> {
        use opentelemetry_proto::tonic::common::v1::any_value::Value;
        span.attributes
            .iter()
            .find(|kv| kv.key == key)
            .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
                Value::IntValue(i) => Some(*i),
                _ => None,
            })
    }

    fn find_string<'a>(
        span: &'a opentelemetry_proto::tonic::trace::v1::Span,
        key: &str,
    ) -> Option<&'a str> {
        use opentelemetry_proto::tonic::common::v1::any_value::Value;
        span.attributes
            .iter()
            .find(|kv| kv.key == key)
            .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
                Value::StringValue(s) => Some(s.as_str()),
                _ => None,
            })
    }

    fn find_bool(span: &opentelemetry_proto::tonic::trace::v1::Span, key: &str) -> Option<bool> {
        use opentelemetry_proto::tonic::common::v1::any_value::Value;
        span.attributes
            .iter()
            .find(|kv| kv.key == key)
            .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
                Value::BoolValue(b) => Some(*b),
                _ => None,
            })
    }

    /// Drive a [`GrpcStream`] to completion (or first error) and return the
    /// collected results.
    async fn drain(stream: GrpcStream) -> Vec<DataFusionResult<RecordBatch>> {
        let mut stream = stream;
        let mut out = Vec::new();
        while let Some(item) = stream.next().await {
            let is_err = item.is_err();
            out.push(item);
            // Stop after the first error — `try_stream!` ends the stream there too.
            if is_err {
                break;
            }
        }
        out
    }

    #[tokio::test]
    async fn pipeline_records_per_batch_stats_and_first_response() {
        let provider = FakeProvider::new(vec![Ok(1), Ok(2), Ok(3)]);
        let pending = make_pending();
        let stream = GrpcStream::execute(&fake_schema(), provider, Some(pending.clone()));

        let items = drain(stream).await;
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|r| r.is_ok()));

        let span = pending.build_span_for_test();
        assert_eq!(find_int(&span, "fetch_grpc_requests"), Some(3));
        assert_eq!(find_int(&span, "num_record_batches"), Some(3));
        assert_eq!(find_int(&span, "rows_returned"), Some(3));
        assert!(
            find_int(&span, "bytes_returned").unwrap() > 0,
            "bytes should reflect arrow array size"
        );
        // First response/batch hooks fire.
        assert!(find_int(&span, "time_to_first_response_us").is_some());
        assert!(find_int(&span, "time_to_first_batch_us").is_some());
        assert_eq!(find_bool(&span, "is_success"), Some(true));
    }

    #[tokio::test]
    async fn pipeline_records_grpc_fetch_error_when_send_request_fails() {
        let provider = FakeProvider::fail_send_request();
        let pending = make_pending();
        let stream = GrpcStream::execute(&fake_schema(), provider, Some(pending.clone()));

        let items = drain(stream).await;
        assert_eq!(items.len(), 1);
        assert!(items[0].is_err());

        let span = pending.build_span_for_test();
        assert_eq!(find_bool(&span, "is_success"), Some(false));
        assert_eq!(find_string(&span, "error_kind"), Some("grpc_fetch"));
        // No batches were produced.
        assert_eq!(find_int(&span, "num_record_batches"), Some(0));
        assert_eq!(find_int(&span, "rows_returned"), Some(0));
        // First-response was never reached.
        assert!(find_int(&span, "time_to_first_response_us").is_none());
    }

    #[tokio::test]
    async fn pipeline_records_grpc_fetch_error_on_stream_item_error() {
        let provider = FakeProvider::new(vec![
            Ok(1),
            Err(ApiError::deserialization(None, "fake mid-stream err")),
        ]);
        let pending = make_pending();
        let stream = GrpcStream::execute(&fake_schema(), provider, Some(pending.clone()));

        let items = drain(stream).await;
        // First batch decoded successfully, second iteration surfaces the error.
        assert_eq!(items.len(), 2);
        assert!(items[0].is_ok());
        assert!(items[1].is_err());

        let span = pending.build_span_for_test();
        assert_eq!(find_bool(&span, "is_success"), Some(false));
        assert_eq!(find_string(&span, "error_kind"), Some("grpc_fetch"));
        // The successful batch before the error is still counted.
        assert_eq!(find_int(&span, "num_record_batches"), Some(1));
        assert_eq!(find_int(&span, "rows_returned"), Some(1));
    }

    #[tokio::test]
    async fn pipeline_records_decode_error() {
        let provider = FakeProvider::fail_decode(vec![1, 2]);
        let pending = make_pending();
        let stream = GrpcStream::execute(&fake_schema(), provider, Some(pending.clone()));

        let items = drain(stream).await;
        assert_eq!(items.len(), 1);
        assert!(items[0].is_err());

        let span = pending.build_span_for_test();
        assert_eq!(find_bool(&span, "is_success"), Some(false));
        assert_eq!(find_string(&span, "error_kind"), Some("decode"));
        // First gRPC message arrived (so first_response was set), but no batch
        // was successfully decoded.
        assert!(find_int(&span, "time_to_first_response_us").is_some());
        assert_eq!(find_int(&span, "num_record_batches"), Some(0));
    }

    #[tokio::test]
    async fn pipeline_propagates_trace_id_into_span() {
        let trace_id = opentelemetry::TraceId::from_bytes([9u8; 16]);
        let provider = FakeProvider::new(vec![Ok(1)]).with_trace_id(trace_id);
        let pending = make_pending();
        let stream = GrpcStream::execute(&fake_schema(), provider, Some(pending.clone()));

        let _ = drain(stream).await;

        let span = pending.build_span_for_test();
        assert_eq!(span.links.len(), 1);
        assert_eq!(span.links[0].trace_id, trace_id.to_bytes().to_vec());
    }

    #[tokio::test]
    async fn pipeline_runs_without_analytics_attached() {
        // No PendingTableQueryAnalytics — recording paths are skipped, but the
        // stream still produces correct output. Smoke test for the
        // `if let Some(analytics) = analytics.as_ref()` branches.
        let provider = FakeProvider::new(vec![Ok(1), Ok(2)]);
        let stream = GrpcStream::execute(&fake_schema(), provider, None);

        let items = drain(stream).await;
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn begin_scan_analytics_default_returns_none() {
        // The default trait impl on `GrpcStreamToTable` returns None — only
        // `TableEntryTableProvider` overrides it. Non-table providers should
        // remain analytics-free.
        let provider = FakeProvider::new(vec![]);
        let schema = fake_schema();
        let result = provider.begin_scan_analytics(&schema, None, None);
        assert!(result.is_none());
    }

    #[test]
    fn build_table_query_span_called_via_pending_matches_pure_builder() {
        // Sanity check: PendingTableQueryAnalytics::build_span_for_test
        // produces equivalent output to calling `build_table_query_span`
        // directly with the same inputs. Pins the wiring between the two.
        let pending = make_pending();
        pending.record_batch(10, 100);
        pending.record_batch(20, 200);
        pending.record_first_response();
        pending.record_first_batch();

        let span_via_pending = pending.build_span_for_test();
        // Required keys present in both forms.
        let direct = build_table_query_span(
            &TableQueryInfo {
                table_id: "tbl-pipeline".to_owned(),
                table_kind: TableKind::Lance,
                caller: TableQueryCaller::CatalogResolver,
                schema_total_columns: 1,
                projected_columns: 1,
                has_limit: false,
                limit_value: None,
                time_range: web_time::SystemTime::now()..web_time::SystemTime::now(),
            },
            crate::analytics::TableScanStatsSnapshot {
                grpc_requests: 2,
                batches: 2,
                rows_returned: 30,
                bytes_returned: 300,
            },
            web_time::SystemTime::now()..web_time::SystemTime::now(),
            std::time::Duration::ZERO,
            None,
            None,
            None,
            None,
            None,
        );

        // Same span name and required-attr counts, regardless of the
        // (slightly different) timing values the wrappers compute.
        assert_eq!(span_via_pending.name, direct.name);
        assert_eq!(find_int(&span_via_pending, "rows_returned"), Some(30));
        assert_eq!(find_int(&span_via_pending, "num_record_batches"), Some(2));
    }
}
