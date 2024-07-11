use std::any::Any;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use arrow2::array::{Arrow2Arrow as _, ListArray};
use datafusion::arrow::array::{ArrayRef, GenericListArray};
use datafusion::arrow::datatypes::{Field, Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::datasource::{TableProvider, TableType};
use datafusion::error::Result;
use datafusion::execution::context::{SessionState, TaskContext};
use datafusion::physical_expr::PhysicalSortExpr;
use datafusion::physical_plan::memory::MemoryStream;
use datafusion::physical_plan::{
    project_schema, DisplayAs, DisplayFormatType, ExecutionPlan, SendableRecordBatchStream,
};
use datafusion::prelude::*;

use async_trait::async_trait;
use re_chunk_store::ChunkStore;
use re_types::Archetype as _;

//use crate::conversions::convert_datatype_arrow2_to_arrow;

/// A custom datasource, used to represent a datastore with a single index
#[derive(Clone)]
pub struct CustomDataSource {
    // TODO(jleibs): Sort out lifetime here so we don't need to take ownership.
    store: ChunkStore,
}

impl CustomDataSource {
    pub fn new(store: ChunkStore) -> Self {
        Self { store }
    }
}

impl Debug for CustomDataSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("custom_db")
    }
}

impl CustomDataSource {
    pub(crate) async fn create_physical_plan(
        &self,
        projections: Option<&Vec<usize>>,
        schema: SchemaRef,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        Ok(Arc::new(CustomExec::try_new(
            projections,
            &schema,
            self.clone(),
        )?))
    }
}

#[async_trait]
impl TableProvider for CustomDataSource {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        // TODO(jleibs): This should come from the chunk store directly
        let components = re_log_types::example_components::MyPoints::all_components();

        let fields: Vec<Field> = components
            .iter()
            .filter_map(|c| {
                self.store.lookup_datatype(c).map(|dt| {
                    Field::new(
                        c.as_str(),
                        ListArray::<i32>::default_datatype(dt.clone()).into(),
                        true,
                    )
                })
            })
            .collect();

        Arc::new(Schema::new(fields))
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        _state: &SessionState,
        projection: Option<&Vec<usize>>,
        // filters and limit can be used here to inject some push-down operations if needed
        _filters: &[Expr],
        _limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        return self.create_physical_plan(projection, self.schema()).await;
    }
}

#[derive(Debug, Clone)]
struct CustomExec {
    _db: CustomDataSource,
    projected_schema: SchemaRef,
}

impl CustomExec {
    fn try_new(
        projections: Option<&Vec<usize>>,
        schema: &SchemaRef,
        db: CustomDataSource,
    ) -> Result<Self> {
        let projected_schema = project_schema(schema, projections)?;
        Ok(Self {
            _db: db,
            projected_schema,
        })
    }
}

impl DisplayAs for CustomExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CustomExec")
    }
}

impl ExecutionPlan for CustomExec {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        self.projected_schema.clone()
    }

    fn output_partitioning(&self) -> datafusion::physical_plan::Partitioning {
        datafusion::physical_plan::Partitioning::UnknownPartitioning(1)
    }

    fn output_ordering(&self) -> Option<&[PhysicalSortExpr]> {
        None
    }

    fn children(&self) -> Vec<Arc<dyn ExecutionPlan>> {
        vec![]
    }

    fn with_new_children(
        self: Arc<Self>,
        _: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        Ok(self)
    }

    fn execute(
        &self,
        _partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        let batches: datafusion::arrow::error::Result<Vec<RecordBatch>> = self
            ._db
            .store
            .iter_chunks()
            .map(|chunk| {
                let components = re_log_types::example_components::MyPoints::all_components();

                RecordBatch::try_new(
                    self.projected_schema.clone(),
                    components
                        .iter()
                        .filter_map(|c| {
                            chunk.components().get(c).map(|c| {
                                let data = c.to_data();
                                let converted = GenericListArray::<i32>::from(data);

                                Arc::new(converted) as ArrayRef
                            })
                        })
                        .collect(),
                )
            })
            .collect();

        Ok(Box::pin(MemoryStream::try_new(
            batches?,
            self.schema(),
            None,
        )?))
    }
}
