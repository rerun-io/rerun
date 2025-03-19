//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

use std::{any::Any, collections::HashMap, pin::Pin, sync::Arc, task::Poll};

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
    client: CatalogServiceClient<Channel>,
}

impl DataFusionConnector {
    pub fn new(client: CatalogServiceClient<Channel>) -> Self {
        Self { client }
    }
}

impl DataFusionConnector {
    pub fn get_datasets(&self) -> Arc<dyn TableProvider> {
        let table_provider: DataSetTableProvider = self.client.clone().into();

        Arc::new(table_provider)
    }
}

#[derive(Debug)]
struct DataSetTableProvider {
    schema: SchemaRef,
    client: CatalogServiceClient<Channel>,
}

impl DataSetTableProvider {
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
}

impl From<CatalogServiceClient<Channel>> for DataSetTableProvider {
    fn from(client: CatalogServiceClient<Channel>) -> Self {
        let schema = Self::create_schema();
        Self { schema, client }
    }
}

#[async_trait]
impl TableProvider for DataSetTableProvider {
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

type FutureFindEntriesResponse = Result<tonic::Response<FindEntriesResponse>, tonic::Status>;

#[derive(Debug)]
struct DataSetPartitionStream {
    schema: SchemaRef,
    client: CatalogServiceClient<Channel>,
}

impl DataSetPartitionStream {
    fn new(schema: &SchemaRef, client: CatalogServiceClient<Channel>) -> Self {
        Self {
            schema: Arc::clone(schema),
            client,
        }
    }
}

impl PartitionStream for DataSetPartitionStream {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        Box::pin(DataSetStream::new(&self.schema, self.client.clone()))
    }
}

struct DataSetStream {
    schema: SchemaRef,
    client: CatalogServiceClient<Channel>,
    is_complete: bool,

    response_future:
        Option<Pin<Box<dyn futures::Future<Output = FutureFindEntriesResponse> + Send>>>,
}

impl DataSetStream {
    fn new(schema: &SchemaRef, client: CatalogServiceClient<Channel>) -> Self {
        Self {
            is_complete: false,
            schema: Arc::clone(schema),
            client,
            response_future: None,
        }
    }
}

impl RecordBatchStream for DataSetStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

impl Stream for DataSetStream {
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

            let future = Box::pin(async move {
                client
                    .find_entries(tonic::Request::new(FindEntriesRequest {
                        filter: Some(EntryFilter {
                            id: None,
                            name: None,
                            entry_type: Some(EntryType::Dataset.into()),
                        }),
                    }))
                    .await
            });

            this.response_future = Some(future);
        }

        let response = match &mut this.response_future {
            Some(s) => s.as_mut(),
            None => {
                return std::task::Poll::Ready(Some(Err(DataFusionError::Execution(
                    "Unable to create Query Catalog request".to_owned(),
                ))))
            }
        };

        let response = match ready!(response.poll(cx)) {
            Ok(r) => r.into_inner(),
            Err(e) => {
                return std::task::Poll::Ready(Some(Err(DataFusionError::External(Box::new(e)))))
            }
        };

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

        this.is_complete = true;

        Poll::Ready(Some(Ok(record_batch)))
    }
}

// #[derive(Debug)]
// pub struct StorageNodeCatalogTable {
//     schema: SchemaRef,
//     client: StorageNodeServiceClient<Channel>,
// }

// impl TryFrom<&CatalogServiceClient<Channel>> for StorageNodeCatalogTable {
//     type Error = anyhow::Error;

//     fn try_from(value: &CatalogServiceClient<Channel>) -> anyhow::Result<Self> {
//         let first_batch = block_on(async {
//             value
//                 .clone()
//                 .find_entries(tonic::Request::new(FindEntriesRequest { filter: None }))
//                 .await
//                 .ok()?
//                 .into_inner()
//         })
//         .ok_or(anyhow::anyhow!(
//             "Unable to get the first batch from the platform"
//         ))??;

//         // let first_batch = first_batch
//         //     .data
//         //     .ok_or_else(|| exec_datafusion_err!("missing DataframePart in QueryCatalogResponse"))?
//         //     .decode()?;

//         // let schema = first_batch.schema();

//         Ok(Self {
//             schema,
//             client: value.clone(),
//         })
//     }
// }

// #[async_trait]
// impl TableProvider for StorageNodeCatalogTable {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }

//     fn schema(&self) -> SchemaRef {
//         Arc::clone(&self.schema)
//     }

//     fn table_type(&self) -> datafusion::datasource::TableType {
//         datafusion::datasource::TableType::Base
//     }

//     async fn scan(
//         &self,
//         _state: &dyn Session,
//         _projection: Option<&Vec<usize>>,
//         _filters: &[Expr],
//         _limit: Option<usize>,
//     ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
//         Ok(Arc::new(QueryCatalogExec::new(
//             Arc::clone(&self.schema),
//             self.client.clone(),
//         )) as Arc<dyn ExecutionPlan>)
//     }
// }

// type FutureCatalogQueryResponse =
//     Result<tonic::Response<tonic::Streaming<QueryCatalogResponse>>, tonic::Status>;

// struct QueryCatalogStream {
//     schema: SchemaRef,
//     client: StorageNodeServiceClient<Channel>,

//     request_future:
//         Option<Pin<Box<dyn futures::Future<Output = FutureCatalogQueryResponse> + Send>>>,
//     stream: Option<tonic::Streaming<QueryCatalogResponse>>,
// }

// impl QueryCatalogStream {
//     fn new(schema: SchemaRef, client: StorageNodeServiceClient<Channel>) -> Self {
//         Self {
//             schema,
//             client,
//             request_future: None,
//             stream: None,
//         }
//     }
// }

// impl RecordBatchStream for QueryCatalogStream {
//     fn schema(&self) -> SchemaRef {
//         Arc::clone(&self.schema)
//     }
// }

// impl Stream for QueryCatalogStream {
//     type Item = DataFusionResult<RecordBatch>;

//     fn poll_next(
//         self: Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Option<Self::Item>> {
//         let this = self.get_mut();

//         if this.request_future.is_none() {
//             let mut client = this.client.clone();

//             let future = Box::pin(async move {
//                 client
//                     .query_catalog(tonic::Request::new(QueryCatalogRequest {
//                         entry: Some(CatalogEntry {
//                             name: "default".to_owned(),
//                         }),
//                         column_projection: None,
//                         filter: None,
//                     }))
//                     .await
//             });

//             this.request_future = Some(future);
//         }

//         if this.stream.is_none() {
//             let request = match &mut this.request_future {
//                 Some(s) => s.as_mut(),
//                 None => {
//                     return std::task::Poll::Ready(Some(Err(DataFusionError::Execution(
//                         "Unable to create Query Catalog request".to_owned(),
//                     ))))
//                 }
//             };

//             let response = match ready!(request.poll(cx)) {
//                 Ok(r) => r.into_inner(),
//                 Err(e) => {
//                     return std::task::Poll::Ready(Some(Err(DataFusionError::External(Box::new(
//                         e,
//                     )))))
//                 }
//             };

//             this.stream = Some(response);
//             // this.stream = Some(ready!(this
//             //     .client
//             //     .query_catalog(tonic::Request::new(QueryCatalogRequest {
//             //         entry: Some(CatalogEntry {
//             //             name: "default".to_owned(),
//             //         }),
//             //         column_projection: None,
//             //         filter: None,
//             //     }))
//             //     .await
//             //     .map_err(|e| exec_datafusion_err!("{e}"))?
//             //     .into_inner()));
//         }

//         match this.stream.as_mut() {
//             Some(stream) => {
//                 let mut stream = stream.map(|streaming_result| {
//                     streaming_result
//                         .and_then(|result| {
//                             result
//                                 .data
//                                 .ok_or_else(|| {
//                                     tonic::Status::internal(
//                                         "missing DataframePart in QueryCatalogResponse",
//                                     )
//                                 })?
//                                 .decode()
//                                 .map_err(|err| tonic::Status::internal(err.to_string()))
//                         })
//                         .map_err(|e| DataFusionError::External(Box::new(e)))
//                 });

//                 stream.poll_next_unpin(cx)
//             }
//             None => std::task::Poll::Ready(None),
//         }
//     }
// }

// #[derive(Debug)]
// struct QueryCatalogExec {
//     props: PlanProperties,
//     client: StorageNodeServiceClient<Channel>,
// }

// impl QueryCatalogExec {
//     fn new(schema: SchemaRef, client: StorageNodeServiceClient<Channel>) -> Self {
//         let props = PlanProperties::new(
//             datafusion::physical_expr::EquivalenceProperties::new(schema),
//             datafusion::physical_plan::Partitioning::UnknownPartitioning(1),
//             datafusion::physical_plan::execution_plan::EmissionType::Incremental,
//             datafusion::physical_plan::execution_plan::Boundedness::Bounded,
//         );

//         Self { props, client }
//     }
// }

// impl datafusion::physical_plan::DisplayAs for QueryCatalogExec {
//     fn fmt_as(
//         &self,
//         _t: datafusion::physical_plan::DisplayFormatType,
//         f: &mut std::fmt::Formatter<'_>,
//     ) -> std::fmt::Result {
//         write!(f, "{}", self.name())
//     }
// }

// impl ExecutionPlan for QueryCatalogExec {
//     fn name(&self) -> &'static str {
//         "QueryCatalogExec"
//     }

//     fn as_any(&self) -> &dyn Any {
//         self
//     }

//     fn properties(&self) -> &PlanProperties {
//         &self.props
//     }

//     fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
//         Vec::default()
//     }

//     fn with_new_children(
//         self: Arc<Self>,
//         _children: Vec<Arc<dyn ExecutionPlan>>,
//     ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
//         Ok(self)
//     }

//     fn execute(
//         &self,
//         _partition: usize,
//         _context: Arc<TaskContext>,
//     ) -> DataFusionResult<SendableRecordBatchStream> {
//         Ok(Box::pin(QueryCatalogStream::new(
//             self.schema(),
//             self.client.clone(),
//         )))
//     }
// }
