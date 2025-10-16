use arrow::datatypes::Schema;
use async_trait::async_trait;
use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::catalog::Session;
use datafusion::common::exec_err;
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::logical_expr::dml::InsertOp;
use datafusion::physical_expr::{EquivalenceProperties, Partitioning};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use datafusion::{
    catalog::TableProvider,
    error::{DataFusionError, Result as DataFusionResult},
};
use futures::Stream;
use tokio::runtime::Handle;
use tonic::IntoStreamingRequest as _;
use tracing::instrument;

use re_log_types::{EntryId, EntryIdOrName};
use re_protos::cloud::v1alpha1::ext::EntryDetails;
use re_protos::cloud::v1alpha1::{
    EntryFilter, EntryKind, FindEntriesRequest, TableInsertMode, WriteTableRequest,
};
use re_protos::cloud::v1alpha1::{GetTableSchemaRequest, ScanTableRequest, ScanTableResponse};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::ConnectionClient;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable};
use crate::wasm_compat::make_future_send;

#[derive(Clone)]
pub struct TableEntryTableProvider {
    client: ConnectionClient,
    table: EntryIdOrName,
    runtime: Handle,

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
    pub fn new(client: ConnectionClient, table: impl Into<EntryIdOrName>, runtime: Handle) -> Self {
        Self {
            client,
            table: table.into(),
            table_id: None,
            runtime,
        }
    }

    pub fn new_entry_list(client: ConnectionClient, runtime: Handle) -> Self {
        Self::new(client, "__entries", runtime)
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> Result<Arc<dyn TableProvider>, DataFusionError> {
        Ok(GrpcStreamProvider::prepare(self).await?)
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
            InsertOp::Append => TableInsertMode::TableInsertAppend,
            InsertOp::Replace => TableInsertMode::TableInsertReplace,
            InsertOp::Overwrite => TableInsertMode::TableInsertOverwrite,
        };
        Ok(Arc::new(
            crate::table_entry_provider::TableEntryWriterExec::new(
                self.client.clone(),
                input,
                num_partitions,
                self.runtime.clone(),
                entry_id,
                insert_op,
            ),
        ))
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

pub struct RecordBatchGrpcOutputStream<S> {
    inner: S,
    grpc_sender: Option<GrpcStreamSender>,
    error_receiver: tokio::sync::oneshot::Receiver<tonic::Status>,
    grpc_error: Option<tonic::Status>,
    insert_op: TableInsertMode,
}

struct GrpcStreamSender {
    sender: tokio::sync::mpsc::UnboundedSender<WriteTableRequest>,
}

impl<S> RecordBatchStream for RecordBatchGrpcOutputStream<S>
where
    S: Stream<Item = Result<RecordBatch, DataFusionError>> + Send + Unpin + 'static,
{
    fn schema(&self) -> SchemaRef {
        Arc::new(Schema::empty())
    }
}

impl<S> RecordBatchGrpcOutputStream<S>
where
    S: Stream<Item = Result<RecordBatch, DataFusionError>> + Send + Unpin + 'static,
{
    pub fn new(
        inner: S,
        client: ConnectionClient,
        runtime: &Handle,
        table_id: EntryId,
        insert_op: TableInsertMode,
    ) -> Self {
        // Create a channel that both we and the gRPC client will use
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Create an oneshot channel for error reporting
        let (error_tx, error_rx) = tokio::sync::oneshot::channel();

        // Start the gRPC streaming call immediately
        runtime.spawn(async move {
            if let Err(err) = async {
                let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
                    .into_streaming_request()
                    .with_entry_id(table_id)?;
                let mut client = client;

                client.inner().write_table(stream).await
            }
            .await
            {
                // Send the error back to the stream
                // Ignore send error if receiver is dropped
                #[expect(clippy::let_underscore_must_use)]
                let _ = error_tx.send(err);
            }
        });

        Self {
            inner,
            grpc_sender: Some(GrpcStreamSender { sender: tx }),
            error_receiver: error_rx,
            grpc_error: None,
            insert_op,
        }
    }
}

impl<S> Stream for RecordBatchGrpcOutputStream<S>
where
    S: Stream<Item = Result<RecordBatch, DataFusionError>> + Send + Unpin,
{
    type Item = Result<RecordBatch, DataFusionError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check for gRPC errors first (only if we haven't already stored one)
        if self.grpc_error.is_none() {
            match Pin::new(&mut self.error_receiver).poll(cx) {
                Poll::Ready(Ok(status)) => {
                    // Store the error for potential future use
                    self.grpc_error = Some(status.clone());
                    // Return the error immediately
                    return Poll::Ready(Some(Err(DataFusionError::External(Box::new(status)))));
                }
                Poll::Ready(Err(_)) | Poll::Pending => {
                    // Oneshot receiver error means sender was dropped (normal completion)
                }
            }
        }

        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(batch))) => {
                // Send to gRPC if we have a sender
                if let Some(ref grpc_sender) = self.grpc_sender {
                    match batch.encode() {
                        Ok(dataframe_part) => {
                            let request = WriteTableRequest {
                                dataframe_part: Some(dataframe_part),
                                insert_mode: self.insert_op.into(),
                            };
                            // Check if channel is still open
                            if grpc_sender.sender.send(request).is_err() {
                                // Channel closed - the gRPC task may have failed
                                // Check if we have a stored error
                                if let Some(status) = self.grpc_error.take() {
                                    return Poll::Ready(Some(Err(DataFusionError::External(
                                        Box::new(status),
                                    ))));
                                } else {
                                    // Channel closed without error - treat as broken pipe
                                    return Poll::Ready(Some(Err(DataFusionError::External(
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::BrokenPipe,
                                            "gRPC stream closed unexpectedly",
                                        )),
                                    ))));
                                }
                            }
                        }
                        Err(err) => {
                            // Conversion error
                            return Poll::Ready(Some(Err(DataFusionError::External(Box::new(
                                std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("Failed to convert batch: {err}"),
                                ),
                            )))));
                        }
                    }
                }

                Poll::Ready(Some(Ok(batch)))
            }
            Poll::Ready(None) => {
                // Drop the sender to signal end of stream
                self.grpc_sender = None;
                Poll::Ready(None)
            }
            other => other,
        }
    }
}
