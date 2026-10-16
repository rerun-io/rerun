//! Catalog entry lookup and creation.

use re_log_types::{EntryId, EntryName};
use re_protos::cloud::v1alpha1::ext::{
    DatasetDetails, DatasetEntry, EntryDetails, EntryDetailsUpdate,
};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_redap_client::{ApiError, ApiErrorKind, ApiResult, ConnectionClient};

use crate::CatalogClient;

impl CatalogClient {
    /// Find all entries matching the given filter.
    pub async fn find_entries(&self, filter: EntryFilter) -> ApiResult<Vec<EntryDetails>> {
        self.client().await?.find_entries(filter).await
    }

    /// Delete the provided entry.
    pub async fn delete_entry(&self, entry_id: EntryId) -> ApiResult {
        self.client().await?.delete_entry(entry_id).await
    }

    /// Update the provided entry.
    pub async fn update_entry(
        &self,
        entry_id: EntryId,
        update: EntryDetailsUpdate,
    ) -> ApiResult<EntryDetails> {
        self.client().await?.update_entry(entry_id, update).await
    }

    /// Create a new dataset entry named `name`, with an id chosen by the server.
    ///
    /// Fails with [`ApiErrorKind::AlreadyExists`] if the name is taken; see
    /// [`Self::find_or_create_dataset`].
    pub async fn create_dataset(&self, name: EntryName) -> ApiResult<DatasetEntry> {
        self.client().await?.create_dataset_entry(name, None).await
    }

    /// Get information on a dataset entry.
    pub async fn read_dataset(&self, entry_id: EntryId) -> ApiResult<DatasetEntry> {
        self.client().await?.read_dataset_entry(entry_id).await
    }

    /// Update the details of a dataset entry.
    pub async fn update_dataset(
        &self,
        entry_id: EntryId,
        details: DatasetDetails,
    ) -> ApiResult<DatasetEntry> {
        self.client()
            .await?
            .update_dataset_entry(entry_id, details)
            .await
    }

    /// Find the dataset named `name`, creating it if it doesn't exist yet.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn find_or_create_dataset(&self, name: &EntryName) -> ApiResult<EntryId> {
        let mut client = self.client().await?;
        if let Some(id) = find_dataset_by_name(&mut client, name).await? {
            return Ok(id);
        }

        match client.create_dataset_entry(name.clone(), None).await {
            Ok(dataset) => Ok(dataset.details.id),

            // Created concurrently between our lookup and our create.
            Err(err) if err.kind == ApiErrorKind::AlreadyExists => {
                find_dataset_by_name(&mut client, name)
                    .await?
                    .ok_or_else(|| {
                        ApiError::invalid_arguments(
                            self.origin(),
                            format!("dataset '{name}' disappeared while registering"),
                        )
                    })
            }

            Err(err) => Err(err),
        }
    }
}

#[tracing::instrument(level = "info", skip_all)]
async fn find_dataset_by_name(
    client: &mut ConnectionClient,
    name: &EntryName,
) -> ApiResult<Option<EntryId>> {
    let entries = match client
        .find_entries(EntryFilter {
            name: Some(name.to_string()),
            // Pass both `entry_kind` (deprecated) and `entry_kinds`
            // to be compatible with old Hub versions.
            // Drop `entry_kind` when no customer has a 0.14 deployment
            // or older of Rerun Hub.
            entry_kind: Some(EntryKind::Dataset.into()),
            entry_kinds: vec![EntryKind::Dataset.into()],
            ..Default::default()
        })
        .await
    {
        Ok(entries) => entries,
        Err(err) if err.kind == ApiErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    Ok(entries.into_iter().next().map(|entry| entry.id))
}
