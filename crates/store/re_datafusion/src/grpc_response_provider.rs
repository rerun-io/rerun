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
use futures::{ready, Stream};

#[async_trait]
pub trait GrpcResponseToTable:
    std::fmt::Debug + 'static + Send + Sync + Clone + std::marker::Unpin
{
    type GrpcResponse;

    fn create_schema() -> SchemaRef;

    fn process_response(
        &mut self,
        response: Self::GrpcResponse,
    ) -> std::task::Poll<Option<DataFusionResult<RecordBatch>>>;

    async fn send_request(&mut self) -> Result<tonic::Response<Self::GrpcResponse>, tonic::Status>;
}

#[derive(Debug)]
pub struct GrpcResponseProvider<T: GrpcResponseToTable> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcResponseToTable> From<T> for GrpcResponseProvider<T> {
    fn from(client: T) -> Self {
        let schema = T::create_schema();
        Self { schema, client }
    }
}

#[async_trait]
impl<T: GrpcResponseToTable> TableProvider for GrpcResponseProvider<T> {
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
            vec![Arc::new(GrpcResponsePartitionStream::new(
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
pub struct GrpcResponsePartitionStream<T: GrpcResponseToTable> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcResponseToTable> GrpcResponsePartitionStream<T> {
    fn new(schema: &SchemaRef, client: T) -> Self {
        Self {
            schema: Arc::clone(schema),
            client,
        }
    }
}

impl<T: GrpcResponseToTable> PartitionStream for GrpcResponsePartitionStream<T> {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        Box::pin(GrpcResponseStream::new(&self.schema, self.client.clone()))
    }
}

type FutureGrpcResponse<T> =
    Pin<Box<dyn Future<Output = Result<tonic::Response<T>, tonic::Status>> + Send>>;

pub struct GrpcResponseStream<T: GrpcResponseToTable> {
    schema: SchemaRef,
    client: T,
    is_complete: bool,
    response_future: Option<FutureGrpcResponse<T::GrpcResponse>>,
}

impl<T: GrpcResponseToTable> GrpcResponseStream<T> {
    fn new(schema: &SchemaRef, client: T) -> Self {
        Self {
            is_complete: false,
            schema: Arc::clone(schema),
            client,
            response_future: None,
        }
    }
}

impl<T: GrpcResponseToTable> RecordBatchStream for GrpcResponseStream<T> {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

impl<T: GrpcResponseToTable> Stream for GrpcResponseStream<T> {
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

            let future = Box::pin(async move { client.send_request().await });

            this.response_future = Some(future);
        }

        let response = match &mut this.response_future {
            Some(s) => s.as_mut(),
            None => {
                return std::task::Poll::Ready(Some(Err(DataFusionError::Execution(
                    "No grpc response received".to_owned(),
                ))))
            }
        };

        let response = match ready!(response.poll(cx)) {
            Ok(r) => r.into_inner(),
            Err(err) => {
                return std::task::Poll::Ready(Some(Err(DataFusionError::External(Box::new(err)))))
            }
        };

        let result = T::process_response(&mut this.client, response);

        if let Poll::Ready(Some(Ok(_))) = &result {
            this.is_complete = true;
        }

        result
    }
}
