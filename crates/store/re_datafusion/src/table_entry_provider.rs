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
use re_redap_client::ConnectionClient;
use tokio::runtime::Handle;
use tracing::instrument;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

#[derive(Clone)]
pub struct TableEntryTableProvider {
    client: ConnectionClient,
    table: EntryIdOrName,
    runtime: Option<Handle>,

    // cache the table id when resolved
    table_id: Option<EntryId>,
}

impl std::fmt::Debug for TableEntryTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableEntryTableProvider")
            .field("table", &self.table)
            .field("table_id", &self.table_id)
            .finish()
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
        }
    }

    pub fn new_entry_list(client: ConnectionClient, runtime: Option<Handle>) -> Self {
        Self::new(client, "__entries", runtime)
    }

    /// This is a convenience function.
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        let provider = GrpcStreamProvider::prepare(self).await?;
        Ok(provider as Arc<dyn TableProvider>)
    }

    #[instrument(skip(self), err)]
    async fn table_id(&mut self) -> Result<EntryId, DataFusionError> {
        if let Some(table_id) = self.table_id {
            return Ok(table_id);
        }

        let table_id = match &self.table {
            EntryIdOrName::Id(entry_id) => *entry_id,

            EntryIdOrName::Name(table_name) => {
                let mut client = self.client.clone();
                let table_name_copy = table_name.clone();

                let entry_details: EntryDetails = make_future_send(async move {
                    Ok(client
                        .inner()
                        .find_entries(FindEntriesRequest {
                            filter: Some(EntryFilter {
                                id: None,
                                name: Some(table_name_copy),
                                entry_kind: Some(EntryKind::Table as i32),
                            }),
                        })
                        .await)
                })
                .await?
                .map_err(|err| DataFusionError::External(Box::new(err)))?
                .into_inner()
                .entries
                .first()
                .ok_or_else(|| {
                    DataFusionError::External(
                        format!("No entry found with name: {table_name}").into(),
                    )
                })?
                .clone()
                .try_into()
                .map_err(|err| DataFusionError::External(Box::new(err)))?;

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

    #[instrument(skip(self), err)]
    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef> {
        let request = GetTableSchemaRequest {
            table_id: Some(self.table_id().await?.into()),
        };

        let mut client = self.client.clone();

        Ok(Arc::new(
            make_future_send(async move { Ok(client.inner().get_table_schema(request).await) })
                .await?
                .map_err(|err| DataFusionError::External(Box::new(err)))?
                .into_inner()
                .schema
                .ok_or(DataFusionError::External(
                    "Schema missing from GetTableSchema response".into(),
                ))?
                .try_into()?,
        ))
    }

    #[instrument(skip(self), err)]
    async fn send_streaming_request(
        &mut self,
    ) -> DataFusionResult<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>> {
        let request = ScanTableRequest {
            table_id: Some(self.table_id().await?.into()),
        };

        let mut client = self.client.clone();

        make_future_send(async move { Ok(client.inner().scan_table(request).await) })
            .await?
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }

    fn process_response(
        &mut self,
        response: Self::GrpcStreamData,
    ) -> DataFusionResult<RecordBatch> {
        response
            .dataframe_part
            .ok_or(DataFusionError::Execution(
                "DataFrame missing from PartitionList response".to_owned(),
            ))?
            .try_into()
            .map_err(|err| DataFusionError::External(Box::new(err)))
    }

    async fn insert_into(
        &self,
        _state: &dyn Session,
        input: Arc<dyn ExecutionPlan>,
        insert_op: InsertOp,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        let num_partitions = input.properties().output_partitioning().partition_count();
        let entry_id = self.clone().table_id().await?;
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

pub struct RecordBatchGrpcOutputStream {
    input_stream: SendableRecordBatchStream,
    grpc_sender: Option<GrpcStreamSender>,
    thread_status: tokio::sync::oneshot::Receiver<re_redap_client::ApiResult>,
    complete: bool,
    grpc_error: Option<tonic::Status>,
}

struct GrpcStreamSender {
    sender: tokio::sync::mpsc::UnboundedSender<RecordBatch>,
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
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Create an oneshot channel for reporting when the thread is complete
        let (thread_status_tx, thread_status_rx) = tokio::sync::oneshot::channel();

        runtime.spawn(async move {
            let shutdown_response = async {
                let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
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
                Poll::Ready(Ok(Err(status))) => {
                    // Store the error for potential future use
                    // Not ideal to throw out the ApiError, but it doesn't impl Clone
                    self.grpc_error = Some(tonic::Status::internal(status.to_string()));
                    // Return the error immediately
                    return Poll::Ready(Some(Err(DataFusionError::External(Box::new(status)))));
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
                    // Check if channel is still open
                    if grpc_sender.sender.send(batch).is_err() {
                        // Channel closed - the gRPC task may have failed
                        // Check if we have a stored error
                        if let Some(status) = self.grpc_error.take() {
                            return Poll::Ready(Some(Err(DataFusionError::External(Box::new(
                                status,
                            )))));
                        } else {
                            // Channel closed without error - treat as broken pipe
                            return Poll::Ready(Some(Err(DataFusionError::External(Box::new(
                                std::io::Error::new(
                                    std::io::ErrorKind::BrokenPipe,
                                    "gRPC stream closed unexpectedly",
                                ),
                            )))));
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
