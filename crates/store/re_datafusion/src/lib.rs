//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

use std::{any::Any, collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll};

use async_trait::async_trait;

use arrow::{
    array::{ArrayRef, Int32Array, Int64Array, RecordBatch, StringArray, StructArray, UInt64Array},
    datatypes::{DataType, Field, Fields, Schema, SchemaRef},
};
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
use itertools::multiunzip;
use re_protos::catalog::v1alpha1::{
    catalog_service_client::CatalogServiceClient, EntryFilter, EntryType, FindEntriesRequest,
    FindEntriesResponse,
};
use tonic::transport::Channel;

pub struct DataFusionConnector {
    catalog: CatalogServiceClient<Channel>,
}

impl DataFusionConnector {
    pub fn new(channel: &Channel) -> Self {
        let catalog = CatalogServiceClient::new(channel.clone());
        Self { catalog }
    }
}

impl DataFusionConnector {
    pub fn get_datasets(&self) -> Arc<dyn TableProvider> {
        let table_provider: DataSetTableProvider<CatalogServiceClient<Channel>> =
            self.catalog.clone().into();

        Arc::new(table_provider)
    }
}

#[async_trait]
trait GrpcTableProvider: std::fmt::Debug + 'static + Send + Sync + Clone + std::marker::Unpin {
    type GrpcResponse;

    fn create_schema() -> SchemaRef;

    fn process_response(
        &mut self,
        response: Self::GrpcResponse,
    ) -> std::task::Poll<Option<DataFusionResult<RecordBatch>>>;

    async fn send_request(&mut self) -> Result<tonic::Response<Self::GrpcResponse>, tonic::Status>;
}

#[derive(Debug)]
struct DataSetTableProvider<T: GrpcTableProvider> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcTableProvider> From<T> for DataSetTableProvider<T> {
    fn from(client: T) -> Self {
        let schema = T::create_schema();
        Self { schema, client }
    }
}

#[async_trait]
impl<T: GrpcTableProvider> TableProvider for DataSetTableProvider<T> {
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
            vec![Arc::new(DataSetPartitionStream::new(
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

type FutureGrpcResponse<T> = Result<tonic::Response<T>, tonic::Status>;

#[derive(Debug)]
struct DataSetPartitionStream<T: GrpcTableProvider> {
    schema: SchemaRef,
    client: T,
}

impl<T: GrpcTableProvider> DataSetPartitionStream<T> {
    fn new(schema: &SchemaRef, client: T) -> Self {
        Self {
            schema: Arc::clone(schema),
            client,
        }
    }
}

impl<T: GrpcTableProvider> PartitionStream for DataSetPartitionStream<T> {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        Box::pin(DataSetStream::new(&self.schema, self.client.clone()))
    }
}

struct DataSetStream<T: GrpcTableProvider> {
    schema: SchemaRef,
    client: T,
    is_complete: bool,

    response_future:
        Option<Pin<Box<dyn Future<Output = FutureGrpcResponse<T::GrpcResponse>> + Send>>>,
}

impl<T: GrpcTableProvider> DataSetStream<T> {
    fn new(schema: &SchemaRef, client: T) -> Self {
        Self {
            is_complete: false,
            schema: Arc::clone(schema),
            client,
            response_future: None,
        }
    }
}

impl<T: GrpcTableProvider> RecordBatchStream for DataSetStream<T> {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

impl<T: GrpcTableProvider> Stream for DataSetStream<T> {
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
            Err(e) => {
                return std::task::Poll::Ready(Some(Err(DataFusionError::External(Box::new(e)))))
            }
        };

        let result = T::process_response(&mut this.client, response);

        if let Poll::Ready(Some(Ok(_))) = &result {
            this.is_complete = true;
        }

        result
    }
}

#[async_trait]
impl GrpcTableProvider for CatalogServiceClient<Channel> {
    type GrpcResponse = FindEntriesResponse;

    fn create_schema() -> SchemaRef {
        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new(
                    "id",
                    DataType::Struct(Fields::from([
                        Arc::new(Field::new("time_ns", DataType::UInt64, true)),
                        Arc::new(Field::new("inc", DataType::UInt64, true)),
                    ])),
                    true,
                ),
                Field::new("name", DataType::Utf8, true),
                Field::new("entry_type", DataType::Int32, true),
                Field::new("created_at", DataType::Int64, true),
                Field::new("updated_at", DataType::Int64, true),
            ],
            HashMap::default(),
        ))
    }

    async fn send_request(&mut self) -> Result<tonic::Response<Self::GrpcResponse>, tonic::Status> {
        self.find_entries(tonic::Request::new(FindEntriesRequest {
            filter: Some(EntryFilter {
                id: None,
                name: None,
                entry_type: Some(EntryType::Dataset.into()),
            }),
        }))
        .await
    }

    fn process_response(
        &mut self,
        response: Self::GrpcResponse,
    ) -> std::task::Poll<Option<DataFusionResult<RecordBatch>>> {
        let (id_time_ns, id_inc, name, entry_type, created_at, updated_at): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = multiunzip(response.entries.into_iter().map(|entry| {
            (
                entry.id.map(|id| id.time_ns),
                entry.id.map(|id| id.inc),
                entry.name,
                entry.entry_type,
                entry
                    .created_at
                    .map(|t| t.seconds * 1_000_000_000 + t.nanos as i64),
                entry
                    .updated_at
                    .map(|t| t.seconds * 1_000_000_000 + t.nanos as i64),
            )
        }));

        let id_time_ns: ArrayRef = Arc::new(UInt64Array::from(id_time_ns));
        let id_inc: ArrayRef = Arc::new(UInt64Array::from(id_inc));
        let name: ArrayRef = Arc::new(StringArray::from(name));
        let entry_type: ArrayRef = Arc::new(Int32Array::from(entry_type));
        let created_at: ArrayRef = Arc::new(Int64Array::from(created_at));
        let updated_at: ArrayRef = Arc::new(Int64Array::from(updated_at));

        let id: ArrayRef = Arc::new(StructArray::from(vec![
            (
                Arc::new(Field::new("time_ns", DataType::UInt64, true)),
                id_time_ns,
            ),
            (Arc::new(Field::new("inc", DataType::UInt64, true)), id_inc),
        ]));

        let record_batch = match RecordBatch::try_from_iter(vec![
            ("id", id),
            ("name", name),
            ("entry_type", entry_type),
            ("created_at", created_at),
            ("updated_at", updated_at),
        ]) {
            Ok(rb) => rb,
            Err(err) => return Poll::Ready(Some(Err(err.into()))),
        };

        Poll::Ready(Some(Ok(record_batch)))
    }
}
