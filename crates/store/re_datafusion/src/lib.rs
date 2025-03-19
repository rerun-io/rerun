//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

use std::{any::Any, pin::Pin, sync::Arc};

use async_trait::async_trait;

use arrow::{array::RecordBatch, datatypes::SchemaRef};
use datafusion::{
    catalog::{Session, TableProvider},
    common::exec_datafusion_err,
    error::{DataFusionError, Result as DataFusionResult},
    execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext},
    physical_plan::{ExecutionPlan, PlanProperties},
    prelude::Expr,
};
use futures::{executor::block_on, ready, Stream, StreamExt as _};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::remote_store::v1alpha1::{
    storage_node_service_client::StorageNodeServiceClient, CatalogEntry, QueryCatalogRequest,
    QueryCatalogResponse,
};
use tonic::transport::Channel;

pub struct DataFusionConnector {
    client: StorageNodeServiceClient<Channel>,
}

impl DataFusionConnector {
    pub fn new(client: StorageNodeServiceClient<Channel>) -> Self {
        Self { client }
    }
}

impl DataFusionConnector {
    pub fn get_catalog(&self) -> anyhow::Result<Arc<dyn TableProvider>> {
        let table_provider = StorageNodeCatalogTable::try_from(&self.client)?;

        Ok(Arc::new(table_provider))
    }
}

#[derive(Debug)]
pub struct StorageNodeCatalogTable {
    schema: SchemaRef,
    client: StorageNodeServiceClient<Channel>,
}

impl TryFrom<&StorageNodeServiceClient<Channel>> for StorageNodeCatalogTable {
    type Error = anyhow::Error;

    fn try_from(value: &StorageNodeServiceClient<Channel>) -> anyhow::Result<Self> {
        let first_batch = block_on(async {
            value
                .clone()
                .query_catalog(tonic::Request::new(QueryCatalogRequest {
                    entry: Some(CatalogEntry {
                        name: "default".to_owned(),
                    }),
                    column_projection: None,
                    filter: None,
                }))
                .await
                .ok()?
                .into_inner()
                .next()
                .await
        })
        .ok_or(anyhow::anyhow!(
            "Unable to get the first batch from the platform"
        ))??;

        let first_batch = first_batch
            .data
            .ok_or_else(|| exec_datafusion_err!("missing DataframePart in QueryCatalogResponse"))?
            .decode()?;

        let schema = first_batch.schema();

        Ok(Self {
            schema,
            client: value.clone(),
        })
    }
}

#[async_trait]
impl TableProvider for StorageNodeCatalogTable {
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
        Ok(Arc::new(QueryCatalogExec::new(
            Arc::clone(&self.schema),
            self.client.clone(),
        )) as Arc<dyn ExecutionPlan>)
    }
}

type FutureCatalogQueryResponse =
    Result<tonic::Response<tonic::Streaming<QueryCatalogResponse>>, tonic::Status>;

struct QueryCatalogStream {
    schema: SchemaRef,
    client: StorageNodeServiceClient<Channel>,

    request_future:
        Option<Pin<Box<dyn futures::Future<Output = FutureCatalogQueryResponse> + Send>>>,
    stream: Option<tonic::Streaming<QueryCatalogResponse>>,
}

impl QueryCatalogStream {
    fn new(schema: SchemaRef, client: StorageNodeServiceClient<Channel>) -> Self {
        Self {
            schema,
            client,
            request_future: None,
            stream: None,
        }
    }
}

impl RecordBatchStream for QueryCatalogStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

impl Stream for QueryCatalogStream {
    type Item = DataFusionResult<RecordBatch>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.request_future.is_none() {
            let mut client = this.client.clone();

            let future = Box::pin(async move {
                client
                    .query_catalog(tonic::Request::new(QueryCatalogRequest {
                        entry: Some(CatalogEntry {
                            name: "default".to_owned(),
                        }),
                        column_projection: None,
                        filter: None,
                    }))
                    .await
            });

            this.request_future = Some(future);
        }

        if this.stream.is_none() {
            let request = match &mut this.request_future {
                Some(s) => s.as_mut(),
                None => {
                    return std::task::Poll::Ready(Some(Err(DataFusionError::Execution(
                        "Unable to create Query Catalog request".to_owned(),
                    ))))
                }
            };

            let response = match ready!(request.poll(cx)) {
                Ok(r) => r.into_inner(),
                Err(e) => {
                    return std::task::Poll::Ready(Some(Err(DataFusionError::External(Box::new(
                        e,
                    )))))
                }
            };

            this.stream = Some(response);
            // this.stream = Some(ready!(this
            //     .client
            //     .query_catalog(tonic::Request::new(QueryCatalogRequest {
            //         entry: Some(CatalogEntry {
            //             name: "default".to_owned(),
            //         }),
            //         column_projection: None,
            //         filter: None,
            //     }))
            //     .await
            //     .map_err(|e| exec_datafusion_err!("{e}"))?
            //     .into_inner()));
        }

        match this.stream.as_mut() {
            Some(stream) => {
                let mut stream = stream.map(|streaming_result| {
                    streaming_result
                        .and_then(|result| {
                            result
                                .data
                                .ok_or_else(|| {
                                    tonic::Status::internal(
                                        "missing DataframePart in QueryCatalogResponse",
                                    )
                                })?
                                .decode()
                                .map_err(|err| tonic::Status::internal(err.to_string()))
                        })
                        .map_err(|e| DataFusionError::External(Box::new(e)))
                });

                stream.poll_next_unpin(cx)
            }
            None => std::task::Poll::Ready(None),
        }
    }
}

#[derive(Debug)]
struct QueryCatalogExec {
    props: PlanProperties,
    client: StorageNodeServiceClient<Channel>,
}

impl QueryCatalogExec {
    fn new(schema: SchemaRef, client: StorageNodeServiceClient<Channel>) -> Self {
        let props = PlanProperties::new(
            datafusion::physical_expr::EquivalenceProperties::new(schema),
            datafusion::physical_plan::Partitioning::UnknownPartitioning(1),
            datafusion::physical_plan::execution_plan::EmissionType::Incremental,
            datafusion::physical_plan::execution_plan::Boundedness::Bounded,
        );

        Self { props, client }
    }
}

impl datafusion::physical_plan::DisplayAs for QueryCatalogExec {
    fn fmt_as(
        &self,
        _t: datafusion::physical_plan::DisplayFormatType,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl ExecutionPlan for QueryCatalogExec {
    fn name(&self) -> &'static str {
        "QueryCatalogExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn properties(&self) -> &PlanProperties {
        &self.props
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        Vec::default()
    }

    fn with_new_children(
        self: Arc<Self>,
        _children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        Ok(self)
    }

    fn execute(
        &self,
        _partition: usize,
        _context: Arc<TaskContext>,
    ) -> DataFusionResult<SendableRecordBatchStream> {
        Ok(Box::pin(QueryCatalogStream::new(
            self.schema(),
            self.client.clone(),
        )))
    }
}
