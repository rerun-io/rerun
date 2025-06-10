use tokio_stream::StreamExt as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::catalog::v1alpha1::ext::{
    CreateDatasetEntryResponse, DatasetDetails, DatasetEntry, EntryDetails,
    ReadDatasetEntryResponse, UpdateDatasetEntryRequest, UpdateDatasetEntryResponse,
};
use re_protos::catalog::v1alpha1::{
    CreateDatasetEntryRequest, DeleteEntryRequest, EntryFilter, FindEntriesRequest,
    ReadDatasetEntryRequest,
};
use re_protos::common::v1alpha1::ext::{IfMissingBehavior, PartitionId, ScanParameters};
use re_protos::frontend::v1alpha1::ext::ScanPartitionTableRequest;
use re_protos::manifest_registry::v1alpha1::ScanPartitionTableResponse;

use crate::{RedapClient, StreamError};

/// Expose an ergonomic API over the gRPC redap client.
// TODO(ab): this should NOT be `Clone`, to discourage callsites from holding on to a client for
// too long. However we have a bunch of places that needs to be fixed before we can do that.
#[derive(Debug, Clone)]
pub struct ConnectionClient(RedapClient);

impl ConnectionClient {
    /// Create a new `ConnectionClient` from a `RedapClient`.
    pub(crate) fn new(client: RedapClient) -> Self {
        Self(client)
    }

    /// Get a mutable reference to the underlying `RedapClient`.
    pub fn inner(&mut self) -> &mut RedapClient {
        &mut self.0
    }
}

// ---

// helpers
impl ConnectionClient {
    pub async fn find_entries(
        &mut self,
        filter: EntryFilter,
    ) -> Result<Vec<EntryDetails>, StreamError> {
        let result = self
            .inner()
            .find_entries(FindEntriesRequest {
                filter: Some(filter),
            })
            .await?
            .into_inner()
            .entries;

        Ok(result
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<EntryDetails>, _>>()?)
    }

    pub async fn delete_entry(&mut self, entry_id: EntryId) -> Result<(), StreamError> {
        self.inner()
            .delete_entry(DeleteEntryRequest {
                id: Some(entry_id.into()),
            })
            .await?;

        Ok(())
    }

    pub async fn create_dataset_entry(
        &mut self,
        name: String,
    ) -> Result<DatasetEntry, StreamError> {
        let response: CreateDatasetEntryResponse = self
            .inner()
            .create_dataset_entry(CreateDatasetEntryRequest { name: Some(name) })
            .await?
            .into_inner()
            .try_into()?;

        Ok(response.dataset)
    }

    pub async fn read_dataset_entry(
        &mut self,
        entry_id: EntryId,
    ) -> Result<DatasetEntry, StreamError> {
        let response: ReadDatasetEntryResponse = self
            .inner()
            .read_dataset_entry(ReadDatasetEntryRequest {
                id: Some(entry_id.into()),
            })
            .await?
            .into_inner()
            .try_into()?;

        Ok(response.dataset_entry)
    }

    pub async fn update_dataset_entry(
        &mut self,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> Result<DatasetEntry, StreamError> {
        let response: UpdateDatasetEntryResponse = self
            .inner()
            .update_dataset_entry(tonic::Request::new(
                UpdateDatasetEntryRequest {
                    id: entry_id,
                    dataset_details,
                }
                .into(),
            ))
            .await?
            .into_inner()
            .try_into()?;

        Ok(response.dataset_entry)
    }

    /// Get a list of partition IDs for the given dataset entry ID.
    pub async fn get_dataset_partition_ids(
        &mut self,
        entry_id: EntryId,
    ) -> Result<Vec<PartitionId>, StreamError> {
        const COLUMN_NAME: &str = ScanPartitionTableResponse::PARTITION_ID;

        let mut stream = self
            .inner()
            .scan_partition_table(tonic::Request::new(
                ScanPartitionTableRequest {
                    dataset_id: entry_id,
                    scan_parameters: Some(ScanParameters {
                        columns: vec![COLUMN_NAME.to_owned()],
                        on_missing_columns: IfMissingBehavior::Error,
                        ..Default::default()
                    }),
                }
                .into(),
            ))
            .await?
            .into_inner();

        let mut partition_ids = Vec::new();

        while let Some(resp) = stream.next().await {
            let record_batch = resp?.data()?.decode()?;

            let partition_id_col = record_batch
                .column_by_name(COLUMN_NAME)
                .ok_or_else(|| StreamError::MissingDataframeColumn(COLUMN_NAME.to_owned()))?;

            let partition_id_array =
                partition_id_col.try_downcast_array_ref::<arrow::array::StringArray>()?;

            partition_ids.extend(
                partition_id_array
                    .iter()
                    .filter_map(|opt| opt.map(|s| PartitionId::new(s.to_owned()))),
            );
        }

        Ok(partition_ids)
    }
}
