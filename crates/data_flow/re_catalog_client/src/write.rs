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

    #[error("failed to upload RRD: {0}")]
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

/// Generate a fresh object key for an RRD upload, relative to catalog storage.
///
/// Keys follow `{storage_prefix}/{write_id}.rrd`; an absent prefix adds no directory.
/// Fails only if the prefix leaves too little room for the write ID within [`ObjectKey::MAX_LEN`].
///
/// Each call generates a new write ID, so replacing a catalog layer need not overwrite
/// an object referenced by an earlier registration.
fn new_segment_key(storage_prefix: Option<&ObjectKey>) -> Result<ObjectKey, Box<WriteError>> {
    let write_id = re_tuid::Tuid::new();
    let key = match storage_prefix {
        None => format!("{write_id}.rrd"),
        Some(prefix) => format!("{prefix}/{write_id}.rrd"),
    };
    ObjectKey::try_new(key).map_err(|err| Box::new(WriteError::Key(err)))
}

impl CatalogClient {
    /// Upload complete RRD bytes and submit their recordings for registration.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    /// An absent prefix adds no directory.
    pub async fn write_segment_bytes(
        &self,
        dataset_id: EntryId,
        data: bytes::Bytes,
        storage_prefix: Option<&ObjectKey>,
        layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<WriteHandle, Box<WriteError>> {
        let key = new_segment_key(storage_prefix)?;
        self.write_segment(dataset_id, data, key, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload a local RRD file and submit its recordings for registration.
    ///
    /// Generates a fresh `{storage_prefix}/{write_id}.rrd` key in catalog storage.
    /// An absent prefix adds no directory.
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
        let key = new_segment_key(storage_prefix)?;
        let file =
            std::fs::File::open(&path).map_err(|source| WriteError::Open { source, path })?;
        self.write_segment(dataset_id, file, key, layer, on_duplicate)
            .await
            .map_err(Box::new)
    }

    /// Upload an RRD and submit its recordings for registration.
    ///
    /// The key must be unique for each write, including replacements.
    /// Dropping the future stops local work but does not cancel submitted registration.
    /// Wait on the returned handle to confirm registration.
    // TODO(RR-2168): Add a segment_id parameter once registration supports overriding encoded recording IDs.
    pub async fn write_segment(
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
