//! Upload and registration lifecycle for finite RRD sources.

use std::path::PathBuf;
use std::time::Duration;

use re_async::AsyncReadAt;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    DataSource, DataSourceKind, InvalidObjectKeyError, ObjectKey,
};
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_redap_client::ApiError;
use re_types_core::LayerName;

use crate::{CatalogClient, RegistrationError, RegistrationHandle, StagingError};

/// An uploaded RRD whose registration may still be running.
#[must_use = "registration continues asynchronously; wait for it"]
#[derive(Clone, Debug)]
pub struct WriteHandle {
    /// The uploaded object's URL, retained even if no new segments are registered.
    pub storage_url: url::Url,
    registration: RegistrationHandle,
}

impl WriteHandle {
    /// Wait for all recordings in the uploaded RRD to be registered.
    pub async fn wait(&self, timeout: Duration) -> Result<Vec<SegmentId>, RegistrationError> {
        self.registration.wait(timeout).await
    }
}

/// A failure uploading or registering an RRD.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("failed to create an upload key: {0}")]
    Key(#[source] InvalidObjectKeyError),

    #[error("failed to open source: {source}\nFile path: {}", .path.display())]
    Open {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },

    #[error(
        "failed to register uploaded RRD: {source}\nDataset: {dataset_id}\nStaged URL: {storage_url}"
    )]
    Register {
        dataset_id: EntryId,
        storage_url: url::Url,
        #[source]
        source: Box<ApiError>,
    },

    #[error("failed to resolve {kind} dataset for write: {source}\nDataset: {dataset_id}")]
    ResolveDataset {
        dataset_id: EntryId,
        kind: &'static str,
        #[source]
        source: Box<ApiError>,
    },

    #[error("failed to upload: {0}")]
    Upload(#[source] StagingError),
}

impl WriteError {
    fn register(dataset_id: EntryId, storage_url: &url::Url, source: ApiError) -> Self {
        Self::Register {
            dataset_id,
            storage_url: storage_url.clone(),
            source: Box::new(source),
        }
    }
}

/// Generate a fresh object key relative to catalog storage.
///
/// Keys follow `{storage_prefix}/{write_id}.{extension}`.
/// Fails only if the prefix leaves too little room for the write ID within [`ObjectKey::MAX_LEN`].
///
/// Each call generates a new write ID, so replacing a catalog layer need not overwrite
/// an object referenced by an earlier registration.
fn new_write_key(
    storage_prefix: Option<&ObjectKey>,
    extension: &str,
) -> Result<ObjectKey, WriteError> {
    let write_id = re_tuid::Tuid::new();
    let key = match storage_prefix {
        None => format!("{write_id}.{extension}"),
        Some(prefix) => format!("{prefix}/{write_id}.{extension}"),
    };
    ObjectKey::try_new(key).map_err(WriteError::Key)
}

impl CatalogClient {
    /// Upload complete RRD bytes and submit their recordings for registration.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    pub async fn write_segment_bytes(
        &self,
        dataset_id: EntryId,
        data: bytes::Bytes,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, Box<WriteError>> {
        self.write_segment(dataset_id, data, storage_prefix, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload a local RRD file and submit its recordings for registration.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    /// Keep the file unchanged until the upload finishes.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn write_segment_path(
        &self,
        dataset_id: EntryId,
        path: PathBuf,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, Box<WriteError>> {
        let file =
            std::fs::File::open(&path).map_err(|source| WriteError::Open { source, path })?;
        self.write_segment(dataset_id, file, storage_prefix, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload an RRD and submit its recordings for registration.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    /// Dropping the future stops local work but does not cancel submitted registration.
    /// Wait on the returned handle to confirm registration.
    // TODO(RR-2168): Add a segment_id parameter once registration supports overriding encoded recording IDs.
    pub async fn write_segment(
        &self,
        dataset_id: EntryId,
        source: impl AsyncReadAt,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, WriteError> {
        let key = new_write_key(storage_prefix, "rrd")?;
        self.upload_and_register(dataset_id, source, key, layer, on_duplicate)
            .await
    }

    /// Upload complete RBL bytes to the dataset's blueprint dataset.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rbl` key in catalog storage.
    /// This does not change the dataset's default blueprints.
    pub async fn write_blueprint_bytes(
        &self,
        dataset_id: EntryId,
        data: bytes::Bytes,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, Box<WriteError>> {
        self.write_blueprint(dataset_id, data, storage_prefix, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload a local RBL file to the dataset's blueprint dataset.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rbl` key in catalog storage.
    /// This does not change the dataset's default blueprints.
    /// Keep the file unchanged until the upload finishes.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn write_blueprint_path(
        &self,
        dataset_id: EntryId,
        path: PathBuf,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, Box<WriteError>> {
        let file =
            std::fs::File::open(&path).map_err(|source| WriteError::Open { source, path })?;
        self.write_blueprint(dataset_id, file, storage_prefix, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload an RBL to the dataset's blueprint dataset and submit registration.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rbl` key in catalog storage.
    /// This does not change the dataset's default blueprints.
    /// Wait on the returned handle to confirm registration.
    pub async fn write_blueprint(
        &self,
        dataset_id: EntryId,
        source: impl AsyncReadAt,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, WriteError> {
        let key = new_write_key(storage_prefix, "rbl")?;
        let blueprint_dataset = self
            .read_dataset(dataset_id)
            .await
            .and_then(|dataset| {
                dataset.dataset_details.blueprint_dataset.ok_or_else(|| {
                    ApiError::invalid_arguments(
                        self.origin(),
                        "a blueprint dataset is not configured for this dataset",
                    )
                })
            })
            .map_err(|source| WriteError::ResolveDataset {
                dataset_id,
                kind: "blueprint",
                source: Box::new(source),
            })?;
        self.upload_and_register(blueprint_dataset, source, key, layer, on_duplicate)
            .await
    }

    /// Upload complete RRD bytes to the dataset's asset dataset.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    /// This preserves existing asset coverage; new assets apply to every segment.
    pub async fn write_asset_bytes(
        &self,
        dataset_id: EntryId,
        data: bytes::Bytes,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, Box<WriteError>> {
        self.write_asset(dataset_id, data, storage_prefix, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload a local RRD file to the dataset's asset dataset.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    /// This preserves existing asset coverage; new assets apply to every segment.
    /// Keep the file unchanged until the upload finishes.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn write_asset_path(
        &self,
        dataset_id: EntryId,
        path: PathBuf,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, Box<WriteError>> {
        let file =
            std::fs::File::open(&path).map_err(|source| WriteError::Open { source, path })?;
        self.write_asset(dataset_id, file, storage_prefix, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload an RRD to the dataset's asset dataset and submit registration.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    /// This preserves existing asset coverage; new assets apply to every segment.
    /// Wait on the returned handle to confirm registration.
    pub async fn write_asset(
        &self,
        dataset_id: EntryId,
        source: impl AsyncReadAt,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, WriteError> {
        let key = new_write_key(storage_prefix, "rrd")?;
        let asset_dataset = self
            .ensure_asset_dataset(dataset_id)
            .await
            .map_err(|source| WriteError::ResolveDataset {
                dataset_id,
                kind: "asset",
                source: Box::new(source),
            })?;
        self.upload_and_register(asset_dataset, source, key, layer, on_duplicate)
            .await
    }

    async fn upload_and_register(
        &self,
        dataset_id: EntryId,
        source: impl AsyncReadAt,
        key: ObjectKey,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, WriteError> {
        let staged_url = self.stage(key, source).await.map_err(WriteError::Upload)?;
        let source = DataSource {
            storage_url: staged_url.clone(),
            is_prefix: false,
            layer,
            kind: DataSourceKind::Rrd,
            object_store_config: None,
        };
        let registration = self
            .register_with_dataset(dataset_id, vec![source], on_duplicate)
            .await
            .map_err(|source| WriteError::register(dataset_id, &staged_url, source))?;
        Ok(WriteHandle {
            storage_url: staged_url,
            registration,
        })
    }
}
