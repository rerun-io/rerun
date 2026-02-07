use std::sync::Arc;

use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use datafusion::catalog::TableProvider;
use datafusion::common::exec_err;
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::error::DataFusionError;
use datafusion::execution::SessionStateBuilder;
use datafusion::logical_expr::dml::InsertOp;
use futures::StreamExt as _;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::EntryKind;
use re_protos::cloud::v1alpha1::ext::{EntryDetails, ProviderDetails, TableEntry};

#[derive(Clone)]
pub enum TableType {
    DataFusionTable(Arc<dyn TableProvider>),
    #[cfg(feature = "lance")]
    LanceDataset(Arc<lance::Dataset>),
}

#[derive(Clone)]
pub struct Table {
    id: EntryId,
    name: String,
    table: TableType,

    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,

    provider_details: ProviderDetails,
}

impl Table {
    pub fn new(
        id: EntryId,
        name: String,
        table: TableType,
        created_at: Option<jiff::Timestamp>,
        provider_details: ProviderDetails,
    ) -> Self {
        Self {
            id,
            name,
            table,
            created_at: created_at.unwrap_or_else(jiff::Timestamp::now),
            updated_at: jiff::Timestamp::now(),
            provider_details,
        }
    }

    pub fn id(&self) -> EntryId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
        self.updated_at = jiff::Timestamp::now();
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
        TableEntry {
            details: EntryDetails {
                id: self.id,
                name: self.name.clone(),
                kind: EntryKind::Table,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },

            provider_details: self.provider_details.clone(),
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
            TableType::LanceDataset(dataset) => {
                Arc::new(lance::datafusion::LanceTableProvider::new(
                    Arc::new(dataset.as_ref().clone()),
                    false,
                    false,
                ))
            }
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
        use lance::dataset::{
            MergeInsertBuilder, WhenMatched, WhenNotMatched, WriteMode, WriteParams,
        };
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
                let key_columns: Vec<_> = dataset
                    .schema()
                    .fields
                    .iter()
                    .filter_map(|field| {
                        if field
                            .metadata
                            .get(re_sorbet::metadata::SORBET_IS_TABLE_INDEX)
                            .is_some_and(|v| v.to_lowercase() == "true")
                        {
                            Some(field.name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut builder = MergeInsertBuilder::try_new(Arc::clone(dataset), key_columns)?;

                let op = builder
                    .when_not_matched(WhenNotMatched::InsertAll)
                    .when_matched(WhenMatched::UpdateAll)
                    .try_build()?;

                let (merge_dataset, _merge_stats) = op.execute_reader(reader).await?;

                *dataset = merge_dataset;
            }
            InsertOp::Overwrite => {
                params.mode = WriteMode::Overwrite;

                let _ =
                    lance::Dataset::write(reader, Arc::new(dataset.as_ref().clone()), Some(params))
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
    pub async fn create_table_entry(
        id: EntryId,
        name: &str,
        url: &url::Url,
        schema: SchemaRef,
    ) -> Result<Self, DataFusionError> {
        use re_protos::cloud::v1alpha1::ext::LanceTable;

        let rb = vec![Ok(RecordBatch::new_empty(Arc::clone(&schema)))];
        let rb = arrow::record_batch::RecordBatchIterator::new(rb.into_iter(), schema);

        let ds = Arc::new(
            lance::Dataset::write(rb, url.as_str(), None)
                .await
                .map_err(|err| DataFusionError::External(err.into()))?,
        );
        let created_at = Some(jiff::Timestamp::now());
        let provider_details = LanceTable {
            table_url: url.clone(),
        };

        Ok(Self::new(
            id,
            name.to_owned(),
            TableType::LanceDataset(ds),
            created_at,
            ProviderDetails::LanceTable(provider_details),
        ))
    }

    #[cfg(not(feature = "lance"))]
    #[expect(clippy::unused_async)]
    pub async fn create_table_entry(
        _id: EntryId,
        _name: &str,
        _url: &url::Url,
        _schema: SchemaRef,
    ) -> Result<Self, DataFusionError> {
        exec_err!("Create table not implemented for bare DataFusion table")
    }
}
