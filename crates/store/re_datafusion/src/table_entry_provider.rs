use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arrow::array::RecordBatch;
use arrow::datatypes::{Schema, SchemaRef};
use async_trait::async_trait;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::{exec_err, not_impl_err};
use datafusion::error::{DataFusionError, Result as DataFusionResult};
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::logical_expr::dml::InsertOp;
use datafusion::physical_expr::{EquivalenceProperties, Partitioning};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use futures::Stream;
use re_log_types::{EntryId, EntryIdOrName};
use re_protos::cloud::v1alpha1::ext::{EntryDetails, TableInsertMode};
use re_protos::cloud::v1alpha1::{
    EntryFilter, EntryKind, FindEntriesRequest, GetTableSchemaRequest, ScanTableRequest,
    ScanTableResponse,
};
use re_redap_client::{ApiError, ApiResult, ConnectionClient};
use tokio::runtime::Handle;
use tracing::instrument;

use crate::IntoDfError as _;
use crate::analytics::TableQueryInfo;
use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;
use crate::{ConnectionAnalytics, PendingTableQueryAnalytics, TableKind, TableQueryCaller};
use re_uri::Origin;

#[derive(Clone)]
pub struct TableEntryTableProvider {
    client: ConnectionClient,
    table: EntryIdOrName,
    runtime: Option<Handle>,

    // cache the table id when resolved
    table_id: Option<EntryId>,

    /// Captured at construction so DataFusion-spawned execution tasks can re-attach
    /// the caller's tracing span — otherwise gRPC spans below surface as root traces.
    parent_span: tracing::Span,

    /// Per-connection analytics sink. `None` ⇒ no analytics emitted for this provider.
    analytics: Option<ConnectionAnalytics>,

    /// What initiated this provider's scans. Defaults to `CatalogResolver`; should
    /// be set explicitly via [`Self::with_caller`] when the caller is known.
    caller: TableQueryCaller,

    /// Underlying provider variant for analytics. Defaults to `Unknown`; set via
    /// [`Self::with_table_kind`] when the caller already has the `ProviderDetails`.
    table_kind: TableKind,
}

impl std::fmt::Debug for TableEntryTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableEntryTableProvider")
            .field("table", &self.table)
            .field("table_id", &self.table_id)
            .finish_non_exhaustive()
    }
}

impl TableEntryTableProvider {
    pub fn new(
        client: ConnectionClient,
        table: impl Into<EntryIdOrName>,
        runtime: Option<Handle>,
    ) -> Self {
        Self {
            client,
            table: table.into(),
            table_id: None,
            runtime,
            parent_span: tracing::Span::current(),
            analytics: None,
            caller: TableQueryCaller::CatalogResolver,
            table_kind: TableKind::Unknown,
        }
    }

    pub fn new_entry_list(client: ConnectionClient, runtime: Option<Handle>) -> Self {
        Self::new(client, "__entries", runtime)
            .with_caller(TableQueryCaller::EntriesTable)
            .with_table_kind(TableKind::SystemEntries)
    }

    /// Enable per-scan analytics for this provider.
    ///
    /// `origin` identifies the cloud the analytics should be sent to. The
    /// caller already holds it (via `ConnectionRegistryHandle::client(origin)`
    /// or similar), so we take it directly to avoid exposing the internal
    /// `ConnectionAnalytics` type.
    ///
    /// Without this call no `cloud_scan_table` span will be emitted.
    pub fn with_analytics(mut self, origin: Origin) -> Self {
        let analytics = ConnectionAnalytics::new(origin, &self.client);

        // Lazy-fetch the server version so subsequent spans can be filtered by
        // cloud build. Same pattern as `DataframeQueryTableProvider::new`.
        let analytics_bg = analytics.clone();
        let mut client_bg = self.client.clone();
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

        self.analytics = Some(analytics);
        self
    }

    /// Set the caller identity recorded in the analytics span. Has no effect
    /// unless [`Self::with_analytics`] is also set.
    pub fn with_caller(mut self, caller: TableQueryCaller) -> Self {
        self.caller = caller;
        self
    }

    /// Set the underlying table kind recorded in the analytics span. Has no
    /// effect unless [`Self::with_analytics`] is also set.
    pub fn with_table_kind(mut self, table_kind: TableKind) -> Self {
        self.table_kind = table_kind;
        self
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }

    #[instrument(skip(self), err, parent = &self.parent_span)]
    async fn table_id(&mut self) -> ApiResult<EntryId> {
        if let Some(table_id) = self.table_id {
            return Ok(table_id);
        }

        let table_id = match &self.table {
            EntryIdOrName::Id(entry_id) => *entry_id,

            EntryIdOrName::Name(table_name) => {
                let mut client = self.client.clone();
                let table_name_copy = table_name.clone();

                let response = make_future_send(async move {
                    client
                        .inner()
                        .find_entries(FindEntriesRequest {
                            filter: Some(EntryFilter {
                                id: None,
                                name: Some(table_name_copy),
                                entry_kind: Some(EntryKind::Table as i32),
                            }),
                        })
                        .await
                        .map_err(|err| ApiError::tonic(err, "/FindEntries failed"))
                })
                .await?;
                let trace_id = re_redap_client::extract_trace_id(response.metadata());

                let entry_details: EntryDetails = response
                    .into_inner()
                    .entries
                    .first()
                    .ok_or_else(|| {
                        ApiError::deserialization(
                            trace_id,
                            format!("No entry found with name: {table_name}"),
                        )
                    })?
                    .clone()
                    .try_into()
                    .map_err(|err: re_protos::TypeConversionError| {
                        ApiError::deserialization_with_source(
                            trace_id,
                            err,
                            "failed decoding /FindEntries response",
                        )
                    })?;

                entry_details.id
            }
        };

        self.table_id = Some(table_id);
        Ok(table_id)
    }
}

#[async_trait]
impl GrpcStreamToTable for TableEntryTableProvider {
    type GrpcStreamData = ScanTableResponse;

    #[instrument(skip(self), err, parent = &self.parent_span)]
    async fn fetch_schema(&mut self) -> ApiResult<SchemaRef> {
        let request = GetTableSchemaRequest {
            table_id: Some(self.table_id().await?.into()),
        };

        let mut client = self.client.clone();

        let response = make_future_send(async move {
            client
                .inner()
                .get_table_schema(request)
                .await
                .map_err(|err| ApiError::tonic(err, "/GetTableSchema failed"))
        })
        .await?;
        let trace_id = re_redap_client::extract_trace_id(response.metadata());

        Ok(Arc::new(
            response
                .into_inner()
                .schema
                .ok_or_else(|| {
                    ApiError::deserialization(
                        trace_id,
                        "Schema missing from GetTableSchema response",
                    )
                })?
                .try_into()
                .map_err(|err: arrow::error::ArrowError| {
                    ApiError::deserialization_with_source(
                        trace_id,
                        err,
                        "failed decoding /GetTableSchema response",
                    )
                })?,
        ))
    }

    #[instrument(skip(self), err, parent = &self.parent_span)]
    async fn send_streaming_request(
        &mut self,
    ) -> ApiResult<re_redap_client::ApiResponseStream<Self::GrpcStreamData>> {
        let request = ScanTableRequest {
            table_id: Some(self.table_id().await?.into()),
        };

        let mut client = self.client.clone();

        let response = make_future_send(async move {
            client
                .inner()
                .scan_table(request)
                .await
                .map_err(|err| ApiError::tonic(err, "/ScanTable failed"))
        })
        .await?;

        Ok(re_redap_client::ApiResponseStream::from_tonic_response(
            response,
            "/ScanTable",
        ))
    }

    fn process_response(&mut self, response: Self::GrpcStreamData) -> ApiResult<RecordBatch> {
        response
            .dataframe_part
            .ok_or_else(|| {
                ApiError::deserialization(None, "DataFrame missing from PartitionList response")
            })?
            .try_into()
            .map_err(|err: re_protos::TypeConversionError| {
                ApiError::deserialization_with_source(
                    None,
                    err,
                    "failed decoding /ScanTable response",
                )
            })
    }

    fn begin_scan_analytics(
        &self,
        schema: &SchemaRef,
        projection: Option<&Vec<usize>>,
        limit: Option<usize>,
    ) -> Option<PendingTableQueryAnalytics> {
        let analytics = self.analytics.as_ref()?;

        // `table_id` is `None` until the first `scan()` resolves it. For
        // name-based providers this is normally cached by the time we get
        // here (the schema fetch in `prepare()` calls `table_id()`).
        let table_id = match (&self.table_id, &self.table) {
            (Some(id), _) | (None, EntryIdOrName::Id(id)) => id.to_string(),
            (None, EntryIdOrName::Name(name)) => name.clone(),
        };

        let schema_total_columns = schema.fields().len() as u32;
        let projected_columns = projection
            .map(|p| p.len() as u32)
            .unwrap_or(schema_total_columns);

        let info = TableQueryInfo {
            table_id,
            table_kind: self.table_kind,
            caller: self.caller,
            schema_total_columns,
            projected_columns,
            has_limit: limit.is_some(),
            limit_value: limit.map(|v| v as u64),
            time_range: web_time::SystemTime::now()..web_time::SystemTime::now(),
        };

        Some(analytics.begin_table_query(info, web_time::Instant::now()))
    }

    async fn insert_into(
        &self,
        _state: &dyn Session,
        input: Arc<dyn ExecutionPlan>,
        insert_op: InsertOp,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        let num_partitions = input.properties().output_partitioning().partition_count();
        let entry_id = self
            .clone()
            .table_id()
            .await
            .map_err(|err| err.into_df_error())?;
        let insert_op = match insert_op {
            InsertOp::Append => TableInsertMode::Append,
            InsertOp::Replace => {
                return not_impl_err!("Replacement operations are not supported");
            }
            InsertOp::Overwrite => TableInsertMode::Overwrite,
        };
        let Some(runtime) = self.runtime.clone() else {
            return exec_err!("Writing to table provider is not supported without tokio runtime");
        };
        Ok(Arc::new(TableEntryWriterExec::new(
            self.client.clone(),
            input,
            num_partitions,
            runtime,
            entry_id,
            insert_op,
        )))
    }
}

#[derive(Debug, Clone)]
struct TableEntryWriterExec {
    client: ConnectionClient,
    props: PlanProperties,
    child: Arc<dyn ExecutionPlan>,
    runtime: Handle,
    table_id: EntryId,
    insert_op: TableInsertMode,
}

impl DisplayAs for TableEntryWriterExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TableEntryWriterExec")
    }
}

impl TableEntryWriterExec {
    fn new(
        client: ConnectionClient,
        child: Arc<dyn ExecutionPlan>,
        default_partitioning: usize,
        runtime: Handle,
        table_id: EntryId,
        insert_op: TableInsertMode,
    ) -> Self {
        Self {
            client,
            props: PlanProperties::new(
                EquivalenceProperties::new(Arc::new(Schema::empty())),
                Partitioning::UnknownPartitioning(default_partitioning),
                EmissionType::Incremental,
                Boundedness::Bounded,
            ),
            child,
            runtime,
            table_id,
            insert_op,
        }
    }
}

impl ExecutionPlan for TableEntryWriterExec {
    fn name(&self) -> &'static str {
        "TableEntryWriterExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn properties(&self) -> &PlanProperties {
        &self.props
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![&self.child]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        if children.len() != 1 {
            return exec_err!(
                "TableEntryWriterExec expects only one child plan. Found {}",
                children.len()
            );
        }

        let mut result = self.as_ref().clone();
        result.child = Arc::clone(&children[0]);

        Ok(Arc::new(result))
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> DataFusionResult<SendableRecordBatchStream> {
        let inner = self.child.execute(partition, context)?;

        let stream = RecordBatchGrpcOutputStream::new(
            inner,
            self.client.clone(),
            &self.runtime,
            self.table_id,
            self.insert_op,
        );

        Ok(Box::pin(stream))
    }
}

struct RecordBatchGrpcOutputStream {
    input_stream: SendableRecordBatchStream,
    grpc_sender: Option<GrpcStreamSender>,
    thread_status: tokio::sync::oneshot::Receiver<ApiResult>,
    complete: bool,
    grpc_error: Option<re_redap_client::ApiError>,
}

struct GrpcStreamSender {
    sender: tokio::sync::mpsc::Sender<RecordBatch>,
}

impl RecordBatchStream for RecordBatchGrpcOutputStream {
    fn schema(&self) -> SchemaRef {
        Arc::new(Schema::empty())
    }
}

impl RecordBatchGrpcOutputStream {
    pub fn new(
        input_stream: SendableRecordBatchStream,
        client: ConnectionClient,
        runtime: &Handle,
        table_id: EntryId,
        insert_op: TableInsertMode,
    ) -> Self {
        // Use a bounded channel to provide backpressure
        let (tx, rx) = tokio::sync::mpsc::channel(256); // This number was taken from thin air

        // Create an oneshot channel for reporting when the thread is complete
        let (thread_status_tx, thread_status_rx) = tokio::sync::oneshot::channel();

        runtime.spawn(async move {
            let shutdown_response = async {
                let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
                let mut client = client;

                client.write_table(stream, table_id, insert_op).await
            }
            .await;

            thread_status_tx.send(shutdown_response).ok();
        });

        Self {
            input_stream,
            grpc_sender: Some(GrpcStreamSender { sender: tx }),
            thread_status: thread_status_rx,
            complete: false,
            grpc_error: None,
        }
    }
}

impl Stream for RecordBatchGrpcOutputStream {
    type Item = Result<RecordBatch, DataFusionError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check for gRPC errors first (only if we haven't already stored one)
        if self.grpc_error.is_none() {
            match Pin::new(&mut self.thread_status).poll(cx) {
                Poll::Ready(Ok(Err(err))) => {
                    // Store the error for potential future use
                    self.grpc_error = Some(err.clone());
                    return Poll::Ready(Some(Err(err.into_df_error())));
                }
                Poll::Ready(Ok(Ok(())) | Err(_)) => {
                    self.complete = true;
                }
                Poll::Pending => {}
            }
        }

        match Pin::new(&mut self.input_stream).poll_next(cx) {
            Poll::Ready(Some(Ok(batch))) => {
                // Send to gRPC if we have a sender
                if let Some(ref grpc_sender) = self.grpc_sender {
                    // Use try_send for backpressure with bounded channel
                    match grpc_sender.sender.try_send(batch) {
                        Ok(()) => {}
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            // Channel is full - apply backpressure by returning Pending
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            // Channel closed - the gRPC task may have failed
                            // Check if we have a stored error
                            if let Some(err) = self.grpc_error.take() {
                                return Poll::Ready(Some(Err(err.into_df_error())));
                            } else {
                                // Channel closed without error - treat as broken pipe
                                return Poll::Ready(Some(Err(ApiError::connection(
                                    "/WriteTable gRPC stream closed unexpectedly",
                                )
                                .into_df_error())));
                            }
                        }
                    }
                }

                Poll::Ready(Some(Ok(RecordBatch::new_empty(Arc::new(Schema::empty())))))
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err))),
            Poll::Ready(None) => {
                // Drop the sender to signal end of stream
                self.grpc_sender = None;
                if self.complete {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
