use std::{any::Any, future::Future, pin::Pin, sync::Arc, task::Poll};

use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::{Session, TableProvider},
    error::{DataFusionError, Result as DataFusionResult},
    execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext},
    physical_plan::{
        streaming::{PartitionStream, StreamingTableExec},
        ExecutionPlan,
    },
    prelude::Expr,
};
use futures::{ready, Stream, StreamExt as _};

#[async_trait]
pub trait GrpcStreamToTable:
    std::fmt::Debug + 'static + Send + Sync + Clone + std::marker::Unpin
{
    type GrpcStreamData;

    fn create_schema() -> SchemaRef;

    fn process_response(&mut self, response: Self::GrpcStreamData)
        -> DataFusionResult<RecordBatch>;

    async fn send_streaming_request(
        &mut self,
    ) -> Result<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>, tonic::Status>;
}

#[derive(Debug)]
pub struct GrpcStreamProvider<T: GrpcStreamToTable> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcStreamToTable> From<T> for GrpcStreamProvider<T> {
    fn from(client: T) -> Self {
        let schema = T::create_schema();
        Self { schema, client }
    }
}

#[async_trait]
impl<T: GrpcStreamToTable> TableProvider for GrpcStreamProvider<T> {
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
        _projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        _limit: Option<usize>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        StreamingTableExec::try_new(
            Arc::clone(&self.schema),
            vec![Arc::new(GrpcStreamPartitionStream::new(
                &self.schema,
                self.client.clone(),
            ))],
            None,
            Vec::default(),
            false,
            None,
        )
        .map(|e| Arc::new(e) as Arc<dyn ExecutionPlan>)
    }
}

#[derive(Debug)]
pub struct GrpcStreamPartitionStream<T: GrpcStreamToTable> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcStreamToTable> GrpcStreamPartitionStream<T> {
    fn new(schema: &SchemaRef, client: T) -> Self {
        Self {
            schema: Arc::clone(schema),
            client,
        }
    }
}

impl<T: GrpcStreamToTable> PartitionStream for GrpcStreamPartitionStream<T> {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        Box::pin(GrpcStream::new(&self.schema, self.client.clone()))
    }
}

type FutureGrpcStream<T> = Pin<
    Box<dyn Future<Output = Result<tonic::Response<tonic::Streaming<T>>, tonic::Status>> + Send>,
>;

pub struct GrpcStream<T: GrpcStreamToTable> {
    schema: SchemaRef,
    client: T,
    is_complete: bool,
    response_future: Option<FutureGrpcStream<T::GrpcStreamData>>,
    stream: Option<tonic::Streaming<T::GrpcStreamData>>,
}

impl<T: GrpcStreamToTable> GrpcStream<T> {
    fn new(schema: &SchemaRef, client: T) -> Self {
        Self {
            is_complete: false,
            schema: Arc::clone(schema),
            client,
            response_future: None,
            stream: None,
        }
    }
}

impl<T: GrpcStreamToTable> RecordBatchStream for GrpcStream<T> {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

impl<T: GrpcStreamToTable> Stream for GrpcStream<T> {
    type Item = DataFusionResult<RecordBatch>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.is_complete {
            return Poll::Ready(None);
        }
        let this = self.get_mut();

        if this.response_future.is_none() {
            let mut client = this.client.clone();

            let future = Box::pin(async move { client.send_streaming_request().await });

            this.response_future = Some(future);
        }

        if this.stream.is_none() {
            let response = match &mut this.response_future {
                Some(s) => s.as_mut(),
                None => {
                    return std::task::Poll::Ready(Some(Err(DataFusionError::Execution(
                        "No grpc response received".to_owned(),
                    ))))
                }
            };

            let stream = match ready!(response.poll(cx)) {
                Ok(r) => r.into_inner(),
                Err(err) => {
                    return std::task::Poll::Ready(Some(Err(DataFusionError::External(Box::new(
                        err,
                    )))))
                }
            };

            this.stream = Some(stream);
        }

        match this.stream.as_mut() {
            Some(stream) => {
                let mut stream = stream.map(|streaming_result| {
                    streaming_result
                        .and_then(|result| {
                            this.client
                                .process_response(result)
                                .map_err(|err| tonic::Status::internal(err.to_string()))
                        })
                        .map_err(|err| DataFusionError::External(Box::new(err)))
                });

                stream.poll_next_unpin(cx)
            }
            None => std::task::Poll::Ready(None),
        }
    }
}
