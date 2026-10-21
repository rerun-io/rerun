//! Catalog table creation, lookup, and writes.

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use re_log_types::{EntryId, EntryName};
use re_protos::cloud::v1alpha1::ext::{TableDetails, TableEntry, TableInsertMode};
use re_redap_client::ApiResult;
use tokio_stream::Stream;

use crate::CatalogClient;

impl CatalogClient {
    /// Register a foreign Lance table to a new table entry in the catalog.
    pub async fn register_table(&self, name: EntryName, url: url::Url) -> ApiResult<TableEntry> {
        self.client().await?.register_table(name, url).await
    }

    /// Create a table entry.
    ///
    /// If `url` is provided, the caller must ensure that it is writable and yet unused.
    pub async fn create_table(
        &self,
        name: EntryName,
        url: Option<url::Url>,
        schema: SchemaRef,
    ) -> ApiResult<TableEntry> {
        self.client()
            .await?
            .create_table_entry(name, url, schema)
            .await
    }

    /// Get information on a table entry.
    pub async fn read_table(&self, entry_id: EntryId) -> ApiResult<TableEntry> {
        self.client().await?.read_table_entry(entry_id).await
    }

    /// Update the details of a table entry.
    pub async fn update_table(
        &self,
        entry_id: EntryId,
        details: TableDetails,
    ) -> ApiResult<TableEntry> {
        self.client()
            .await?
            .update_table_entry(entry_id, details)
            .await
    }

    /// Write the batches of `stream` to a table entry.
    ///
    /// `insert_mode` decides how the new rows combine with the rows already in the table.
    pub async fn write_table(
        &self,
        stream: impl Stream<Item = RecordBatch> + Send + 'static,
        entry_id: EntryId,
        insert_mode: TableInsertMode,
    ) -> ApiResult {
        self.client()
            .await?
            .write_table(stream, entry_id, insert_mode)
            .await
    }
}
