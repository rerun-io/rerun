//! Uploading an `.rrd` from the user's machine and registering it as an asset.

use re_catalog_client::{
    AssetCoverage, AssetRegistrationError, CatalogClient, StagingError, WriteError, WriteHandle,
};
use re_log_types::EntryId;
use re_log_types::external::re_types_core::{LayerName, SegmentId};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::{ApiError, ApiErrorKind};

/// Upload an `.rrd` to the catalog's storage and register it as an asset of a dataset, the same
/// way [`CatalogClient::register_asset`] does.
///
/// Every call uploads under a new key. Only a server that advertised
/// [`re_protos::capabilities::CATALOG_WRITE_STAGING`] takes the upload.
pub trait UploadAssetExt {
    /// Upload the `.rrd` at `path`.
    ///
    /// Keep the file unchanged until the upload finishes.
    #[cfg(not(target_arch = "wasm32"))]
    fn upload_asset_path(
        &self,
        dataset_id: EntryId,
        path: std::path::PathBuf,
        coverage: Option<&AssetCoverage>,
        timeout: std::time::Duration,
    ) -> impl Future<Output = Result<SegmentId, AssetRegistrationError>> + Send;

    /// Upload the `.rrd` read from `source`.
    #[cfg(target_arch = "wasm32")]
    fn upload_asset(
        &self,
        dataset_id: EntryId,
        source: impl re_async::AsyncReadAt,
        coverage: Option<&AssetCoverage>,
        timeout: std::time::Duration,
    ) -> impl Future<Output = Result<SegmentId, AssetRegistrationError>>;
}

impl UploadAssetExt for CatalogClient {
    #[cfg(not(target_arch = "wasm32"))]
    async fn upload_asset_path(
        &self,
        dataset_id: EntryId,
        path: std::path::PathBuf,
        coverage: Option<&AssetCoverage>,
        timeout: std::time::Duration,
    ) -> Result<SegmentId, AssetRegistrationError> {
        let written = self
            .write_asset_path(
                dataset_id,
                path,
                None,
                LayerName::base(),
                IfDuplicateBehavior::Overwrite,
            )
            .await
            .map_err(|err| *err);

        wait_for_uploaded_asset(self, written, coverage, timeout).await
    }

    #[cfg(target_arch = "wasm32")]
    async fn upload_asset(
        &self,
        dataset_id: EntryId,
        source: impl re_async::AsyncReadAt,
        coverage: Option<&AssetCoverage>,
        timeout: std::time::Duration,
    ) -> Result<SegmentId, AssetRegistrationError> {
        let written = self
            .write_asset(
                dataset_id,
                source,
                None,
                LayerName::base(),
                IfDuplicateBehavior::Overwrite,
            )
            .await;

        wait_for_uploaded_asset(self, written, coverage, timeout).await
    }
}

/// Write the `coverage` of an uploaded asset, then wait for it to be registered.
async fn wait_for_uploaded_asset(
    client: &CatalogClient,
    written: Result<WriteHandle, WriteError>,
    coverage: Option<&AssetCoverage>,
    timeout: std::time::Duration,
) -> Result<SegmentId, AssetRegistrationError> {
    let origin = client.origin();

    let handle = written.map_err(|err| {
        AssetRegistrationError::without_asset(match err {
            WriteError::Register { source, .. } | WriteError::ResolveDataset { source, .. } => {
                *source
            }
            WriteError::Upload(StagingError::Api(err)) => err,
            err => ApiError::internal_with_source(origin, None, err, "failed to upload asset"),
        })
    })?;

    // TODO(RR-6283): Read the store ids from the file before uploading and pass the chosen one as
    // the `store_id` of the source. The OSS server registers every recording of the file, and only
    // the first one gets the coverage.
    let asset_id = handle.segment_ids().next().cloned().ok_or_else(|| {
        AssetRegistrationError::without_asset(ApiError::internal(
            origin,
            "the server accepted the asset but returned no segment",
        ))
    })?;

    // Properties are stored by segment id, whether or not the segment is registered, so the
    // coverage is in place by the time the asset is.
    if let Some(coverage) = coverage {
        // Uploading the same recording again registers the same asset, so this overwrites the
        // coverage stored for it.
        let written = client
            .set_segment_properties(
                handle.dataset_id,
                asset_id.clone(),
                &coverage.to_properties(),
                None,
            )
            .await;

        match written {
            Ok(_) => {}

            // A server without segment properties loads every asset into every segment.
            Err(err) if err.kind == ApiErrorKind::Unimplemented => {
                re_log::warn!(
                    "The server does not store asset coverage, so the asset applies to every segment"
                );
            }

            Err(err) => return Err(AssetRegistrationError::new(Some(asset_id), err)),
        }
    }

    handle
        .wait(timeout)
        .await
        .map_err(|err| AssetRegistrationError::new(Some(asset_id.clone()), err.into()))?;

    Ok(asset_id)
}
