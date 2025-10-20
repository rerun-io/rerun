use arrow::datatypes::{Schema, SchemaRef};
use arrow::record_batch::{RecordBatch, RecordBatchIterator};
use datafusion::catalog::TableProvider;
use datafusion::common::exec_err;
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::error::DataFusionError;
use datafusion::execution::SessionStateBuilder;
use datafusion::logical_expr::dml::InsertOp;
use futures::StreamExt as _;
use lance::Dataset as LanceDataset;
use lance::datafusion::LanceTableProvider;
use lance::dataset::{WriteMode, WriteParams};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{
    EntryKind,
    ext::{EntryDetails, ProviderDetails as _, SystemTable, TableEntry},
};
use std::sync::Arc;

#[derive(Clone)]
pub enum TableType {
    DataFusionTable(Arc<dyn TableProvider>),
    LanceDataset(Arc<LanceDataset>),
}

#[derive(Clone)]
pub struct Table {
    id: EntryId,
    name: String,
    table: TableType,

    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,

    system_table: Option<SystemTable>,
}

impl Table {
    pub fn new(
        id: EntryId,
        name: String,
        table: TableType,
        created_at: Option<jiff::Timestamp>,
        system_table: Option<SystemTable>,
    ) -> Self {
        Self {
            id,
            name,
            table,
            created_at: created_at.unwrap_or_else(jiff::Timestamp::now),
            updated_at: jiff::Timestamp::now(),
            system_table,
        }
    }

    pub fn id(&self) -> EntryId {
        self.id
    }

    pub fn created_at(&self) -> jiff::Timestamp {
        self.created_at
    }

    pub fn as_entry_details(&self) -> EntryDetails {
        EntryDetails {
            id: self.id,
            name: self.name.clone(),
            kind: EntryKind::Table,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn as_table_entry(&self) -> TableEntry {
        let provider_details = match &self.system_table {
            Some(s) => s.try_as_any().expect("system_table should always be valid"),
            None => Default::default(),
        };

        TableEntry {
            details: EntryDetails {
                id: self.id,
                name: self.name.clone(),
                kind: EntryKind::Table,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },

            provider_details,
        }
    }

    pub fn schema(&self) -> SchemaRef {
        match &self.table {
            TableType::DataFusionTable(t) => t.schema(),
            TableType::LanceDataset(dataset) => Arc::new(Schema::from(dataset.schema())),
        }
    }

    pub fn provider(&self) -> Arc<dyn TableProvider> {
        match &self.table {
            TableType::DataFusionTable(t) => Arc::clone(t),
            TableType::LanceDataset(dataset) => Arc::new(LanceTableProvider::new(
                Arc::new(dataset.as_ref().clone()),
                false,
                false,
            )),
        }
    }

    async fn write_table_provider(
        &self,
        rb: RecordBatch,
        insert_op: InsertOp,
    ) -> Result<(), DataFusionError> {
        let schema = rb.schema();
        let TableType::DataFusionTable(provider) = &self.table else {
            return exec_err!("Expected DataFusion Table Provider");
        };

        let input = MemorySourceConfig::try_new_from_batches(schema, vec![rb])?;
        let session = SessionStateBuilder::default().build();
        let result = provider.insert_into(&session, input, insert_op).await?;
        let mut output = result.execute(0, session.task_ctx())?;

        while let Some(r) = output.next().await {
            let _ = r?;
        }
        Ok(())
    }

    async fn write_table_lance_dataset(
        &mut self,
        rb: RecordBatch,
        insert_op: InsertOp,
    ) -> Result<(), DataFusionError> {
        let schema = rb.schema();
        let mut params = WriteParams::default();

        let TableType::LanceDataset(dataset) = &mut self.table else {
            return exec_err!("Expected Lance Dataset");
        };

        let reader = RecordBatchIterator::new(vec![Ok(rb)], schema);

        match insert_op {
            InsertOp::Append => {
                params.mode = WriteMode::Append;

                dataset
                    .as_ref()
                    .clone()
                    .append(reader, Some(params))
                    .await
                    .map_err(|err| DataFusionError::External(err.into()))?;
            }
            InsertOp::Replace => {
                exec_err!("Invalid insert operation. Only append and overwrite are supported.")?;
            }
            InsertOp::Overwrite => {
                params.mode = WriteMode::Overwrite;

                let _ =
                    LanceDataset::write(reader, Arc::new(dataset.as_ref().clone()), Some(params))
                        .await
                        .map_err(|err| DataFusionError::External(err.into()))?;
            }
        }

        let updated_table = Arc::new(
            lance::Dataset::open(dataset.uri())
                .await
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?,
        );

        self.table = TableType::LanceDataset(updated_table);
        Ok(())
    }

    pub async fn write_table(
        &mut self,
        rb: RecordBatch,
        insert_op: InsertOp,
    ) -> Result<(), DataFusionError> {
        match &self.table {
            TableType::LanceDataset(_) => self.write_table_lance_dataset(rb, insert_op).await,
            TableType::DataFusionTable(_) => self.write_table_provider(rb, insert_op).await,
        }
    }
}
