use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use datafusion::catalog::TableProvider;
use datafusion::common::exec_err;
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::error::DataFusionError;
use datafusion::execution::SessionStateBuilder;
use datafusion::logical_expr::dml::InsertOp;
use futures::StreamExt as _;
#[cfg(feature = "lance")]
use lance::{
    Dataset as LanceDataset,
    datafusion::LanceTableProvider,
    dataset::{WriteMode, WriteParams},
};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{
    EntryKind,
    ext::{EntryDetails, ProviderDetails as _, SystemTable, TableEntry},
};
use std::sync::Arc;

#[derive(Clone)]
pub enum TableType {
    DataFusionTable(Arc<dyn TableProvider>),
    #[cfg(feature = "lance")]
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
            #[cfg(feature = "lance")]
            TableType::LanceDataset(dataset) => {
                Arc::new(arrow::datatypes::Schema::from(dataset.schema()))
            }
        }
    }

    pub fn provider(&self) -> Arc<dyn TableProvider> {
        match &self.table {
            TableType::DataFusionTable(t) => Arc::clone(t),
            #[cfg(feature = "lance")]
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

        #[cfg_attr(not(feature = "lance"), expect(irrefutable_let_patterns))]
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

    #[cfg(feature = "lance")]
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

        let reader = arrow::record_batch::RecordBatchIterator::new(vec![Ok(rb)], schema);

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

    #[cfg_attr(not(feature = "lance"), expect(clippy::needless_pass_by_ref_mut))]
    pub async fn write_table(
        &mut self,
        rb: RecordBatch,
        insert_op: InsertOp,
    ) -> Result<(), DataFusionError> {
        match &self.table {
            #[cfg(feature = "lance")]
            TableType::LanceDataset(_) => self.write_table_lance_dataset(rb, insert_op).await,
            TableType::DataFusionTable(_) => self.write_table_provider(rb, insert_op).await,
        }
    }

    #[cfg(feature = "lance")]
    pub async fn create_table(
        id: EntryId,
        name: &str,
        path: &str,
        schema: SchemaRef,
    ) -> Result<Self, DataFusionError> {
        let rb = vec![Ok(RecordBatch::new_empty(Arc::clone(&schema)))];
        let rb = arrow::record_batch::RecordBatchIterator::new(rb.into_iter(), schema);

        let ds = Arc::new(
            lance::Dataset::write(rb, path, None)
                .await
                .map_err(|err| DataFusionError::External(err.into()))?,
        );
        let created_at = Some(jiff::Timestamp::now());

        Ok(Self::new(
            id,
            name.to_owned(),
            TableType::LanceDataset(ds),
            created_at,
            None,
        ))
    }

    #[cfg(not(feature = "lance"))]
    #[expect(clippy::unused_async)]
    pub async fn create_table(
        _id: EntryId,
        _name: &str,
        _path: &str,
        _schema: SchemaRef,
    ) -> Result<Self, DataFusionError> {
        exec_err!("Create table not implemented for bare DataFusion table")
    }
}
