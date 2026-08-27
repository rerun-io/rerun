use re_log_types::{EntryId, EntryName};
use re_protos::cloud::v1alpha1::ext::{DataSource, RegisterWithDatasetRequest};
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_protos::headers::RerunHeadersInjectorExt as _;

use crate::registration_handle::parse_task_descriptors;
use crate::{
    ApiError, ApiResult, Asset, Connection, ConnectionClient, ConnectionRegistryHandle,
    RegistrationHandle, extract_trace_id,
};

/// Binds a [`ConnectionRegistryHandle`] to one catalog origin.
///
/// The registry owns shared connection and credential state. This handle supplies the origin and
/// owns origin-sensitive workflows, such as registration. Use [`Self::client`] to acquire a
/// short-lived [`ConnectionClient`] for individual RPCs.
#[derive(Clone)]
pub struct ConnectionHandle {
    origin: re_uri::Origin,
    registry: ConnectionRegistryHandle,
}

impl ConnectionHandle {
    pub fn new(registry: ConnectionRegistryHandle, origin: re_uri::Origin) -> Self {
        Self { origin, registry }
    }

    pub async fn connection(&self) -> ApiResult<Connection> {
        self.registry.connection(self.origin.clone()).await
    }

    pub async fn client(&self) -> ApiResult<ConnectionClient> {
        Ok(self.connection().await?.client)
    }

    /// Scan an asset dataset's manifest, one [`Asset`] per asset, sorted by asset id.
    pub async fn scan_asset_dataset(&self, asset_dataset: EntryId) -> ApiResult<Vec<Asset>> {
        let batches = self
            .client()
            .await?
            .scan_dataset_manifest(asset_dataset, crate::asset::ASSET_COLUMNS)
            .await?;

        crate::asset::assets_from_manifest(&self.origin, &batches)
    }

    /// Initiate asynchronous registration of the provided data sources with a dataset.
    pub async fn register_with_dataset(
        &self,
        dataset_id: EntryId,
        data_sources: Vec<DataSource>,
        on_duplicate: IfDuplicateBehavior,
    ) -> ApiResult<RegistrationHandle> {
        let req = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        })
        .with_entry_id(dataset_id);

        let response = self
            .client()
            .await?
            .inner()
            .register_with_dataset(req.map(Into::into))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/RegisterWithDataset failed"))?;
        let trace_id = extract_trace_id(response.metadata());
        let descriptors =
            parse_task_descriptors(&self.origin, trace_id, response.into_inner().data)?;

        Ok(RegistrationHandle::new(self.clone(), trace_id, descriptors))
    }

    /// Ensure a dataset exists and register `data_sources` with it.
    ///
    /// `timeout` applies to the task-completion query.
    pub async fn ensure_dataset_and_register(
        &self,
        dataset_name: &EntryName,
        data_sources: Vec<DataSource>,
        on_duplicate: IfDuplicateBehavior,
        timeout: std::time::Duration,
    ) -> ApiResult<(EntryId, SegmentId)> {
        let dataset_id = self
            .client()
            .await?
            .find_or_create_dataset(dataset_name)
            .await?;
        let registration = self
            .register_with_dataset(dataset_id, data_sources, on_duplicate)
            .await?;
        let segment_id = registration
            .wait(timeout)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                ApiError::invalid_arguments(
                    &self.origin,
                    "server registered the file but returned no segments",
                )
            })?;

        Ok((dataset_id, segment_id))
    }

    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    pub fn connection_registry(&self) -> &ConnectionRegistryHandle {
        &self.registry
    }
}

impl std::fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
