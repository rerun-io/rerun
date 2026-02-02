use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;

use arrow::array::RecordBatch;
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
use tokio_stream::Stream;

#[async_trait]
pub trait GrpcStreamToTable:
    std::fmt::Debug + 'static + Send + Sync + Clone + std::marker::Unpin
{
    type GrpcStreamData;

    async fn fetch_schema(&mut self) -> DataFusionResult<SchemaRef>;

    fn process_response(&mut self, response: Self::GrpcStreamData)
    -> DataFusionResult<RecordBatch>;

    async fn send_streaming_request(
        &mut self,
    ) -> DataFusionResult<tonic::Response<tonic::Streaming<Self::GrpcStreamData>>>;

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
}

#[derive(Debug)]
pub struct GrpcStreamProvider<T: GrpcStreamToTable> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcStreamToTable> GrpcStreamProvider<T> {
    pub async fn prepare(mut client: T) -> Result<Arc<Self>, DataFusionError> {
        let schema = client.fetch_schema().await?;
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
        _limit: Option<usize>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        StreamingTableExec::try_new(
            self.schema.clone(),
            vec![Arc::new(GrpcStreamPartitionStream::new(
                &self.schema,
                self.client.clone(),
            ))],
            projection,
            Vec::default(),
            false,
            None,
        )
        .map(|e| Arc::new(e) as Arc<dyn ExecutionPlan>)
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
}

impl<T: GrpcStreamToTable> GrpcStreamPartitionStream<T> {
    fn new(schema: &SchemaRef, client: T) -> Self {
        Self {
            schema: Arc::clone(schema),
            client,
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
        Box::pin(GrpcStream::execute(&self.schema, self.client.clone()))
    }
}

pub struct GrpcStream {
    schema: SchemaRef,
    adapted_stream: Pin<Box<dyn Stream<Item = datafusion::common::Result<RecordBatch>> + Send>>,
}

impl GrpcStream {
    fn execute<T>(schema: &SchemaRef, mut client: T) -> Self
    where
        T::GrpcStreamData: Send + 'static,
        T: GrpcStreamToTable + Send + 'static,
    {
        let adapted_stream = Box::pin(async_stream::try_stream! {
            let mut stream = client.send_streaming_request().await.map_err(|err| DataFusionError::External(Box::new(
                tonic::Status::internal(err.to_string())
            )))?.into_inner();

            while let Some(msg) = stream.next().await {
                let msg = msg.map_err(|err| DataFusionError::External(Box::new(err)))?;
                let processed = client
                    .process_response(msg)
                    .map_err(|err| DataFusionError::External(Box::new(
                        tonic::Status::internal(err.to_string())
                    )))?;
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
