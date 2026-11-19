//! Catalog assets and their registration lifecycle.

use arrow::array::RecordBatch;
use arrow::datatypes::Schema as ArrowSchema;

use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    AssetMode, DataSource, asset_properties, read_asset_mode, read_asset_segments,
};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::{ApiError, ApiErrorKind, ApiResult};
use re_types_core::SegmentId;

use crate::{CatalogClient, ManifestSegment};

/// The asset dataset of a dataset and the assets in it the server is done registering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredAssets {
    pub asset_dataset: EntryId,

    /// Sorted by asset id.
    pub assets: Vec<ManifestSegment>,
}

/// Which segments of a dataset load an asset.
///
/// Stored in the asset's `asset` property. An asset the server stored no coverage for applies to
/// every segment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetCoverage {
    pub mode: AssetMode,

    /// The segment ids the mode applies to.
    pub segments: Vec<SegmentId>,
}

impl AssetCoverage {
    /// The coverage an asset's properties hold.
    ///
    /// The properties hold the segment ids as a set, so they are sorted here to read the same way
    /// every time.
    pub fn from_properties(properties: &RecordBatch) -> Self {
        let mut segments: Vec<SegmentId> = read_asset_segments(properties, 0)
            .into_iter()
            .map(SegmentId::new)
            .collect();
        segments.sort();

        Self {
            mode: read_asset_mode(properties, 0),
            segments,
        }
    }

    /// The properties holding this coverage.
    pub fn to_properties(&self) -> RecordBatch {
        asset_properties(self.mode, self.segments.iter().map(SegmentId::as_str))
    }
}

/// How long to wait for the server to register or unregister an asset.
pub const DEFAULT_ASSET_TASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// What went wrong registering an asset.
#[derive(Debug)]
pub struct AssetRegistrationError {
    /// The asset the server started registering, if it got that far.
    ///
    /// The server writes the asset's row before it reads the `.rrd` and keeps a failed asset
    /// listed, so this is the asset to unregister to clean up after the failure.
    pub asset_id: Option<SegmentId>,

    pub error: Box<ApiError>,
}

impl AssetRegistrationError {
    /// A failure from before the server started registering an asset, so nothing is registered.
    pub fn without_asset(error: ApiError) -> Self {
        Self::new(None, error)
    }

    /// A failure while registering `asset_id`, or before registering started when it is `None`.
    pub fn new(asset_id: Option<SegmentId>, error: ApiError) -> Self {
        Self {
            asset_id,
            error: Box::new(error),
        }
    }
}

impl std::fmt::Display for AssetRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

/// The data source an asset is registered from, which is always a single `.rrd`.
pub fn asset_data_source(
    origin: &re_uri::Origin,
    asset_uri: impl AsRef<str>,
) -> ApiResult<DataSource> {
    DataSource::new_rrd(asset_uri, None).map_err(|err| {
        ApiError::invalid_arguments_with_source(origin, None, err, "invalid asset url")
    })
}

impl CatalogClient {
    /// Get the asset segments within an asset dataset.
    ///
    /// `segment_id` narrows the result to the assets that apply to that segment, honoring each
    /// asset's mode and segment list. When it is `None`, the per-asset defaults apply with no
    /// segment-specific opt-out/opt-in overrides.
    pub async fn get_assets_for_segment(
        &self,
        asset_dataset: EntryId,
        segment_id: Option<&SegmentId>,
    ) -> ApiResult<Vec<SegmentId>> {
        self.client()
            .await?
            .get_assets_for_segment(asset_dataset, segment_id)
            .await
    }

    /// Scan an asset dataset's manifest, one [`ManifestSegment`] per asset, sorted by asset id.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn scan_asset_dataset(
        &self,
        asset_dataset: EntryId,
    ) -> ApiResult<Vec<ManifestSegment>> {
        self.scan_manifest(asset_dataset, None).await
    }

    /// Register an existing `.rrd` that the server can reach as an asset of `dataset_id`, and wait
    /// for the registration to finish.
    ///
    /// Registering an asset that is already registered replaces its data. `coverage` replaces the
    /// coverage the server stored for it, and with no `coverage` the stored one stays as it is. A
    /// server without segment properties ignores `coverage`, and the asset applies to every segment.
    ///
    /// The server has restrictions on what an asset can contain; See
    /// [`re_protos::common::v1alpha1::ext::DatasetKind::limits`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn register_asset(
        &self,
        dataset_id: EntryId,
        asset_uri: impl AsRef<str>,
        coverage: Option<&AssetCoverage>,
        timeout: std::time::Duration,
    ) -> Result<SegmentId, AssetRegistrationError> {
        let data_source = crate::asset_data_source(self.origin(), asset_uri)
            .map_err(AssetRegistrationError::without_asset)?;

        let asset_dataset = self
            .ensure_asset_dataset(dataset_id)
            .await
            .map_err(AssetRegistrationError::without_asset)?;

        let registration = self
            .register_with_dataset(
                asset_dataset,
                vec![data_source],
                IfDuplicateBehavior::Overwrite,
            )
            .await
            .map_err(AssetRegistrationError::without_asset)?;

        // The server writes the asset's row before it reads the `.rrd`, so its id is known even
        // when reading it fails further down.
        let asset_id = registration
            .descriptors()
            .first()
            .map(|descriptor| descriptor.segment_id.clone())
            .ok_or_else(|| {
                AssetRegistrationError::without_asset(ApiError::internal(
                    self.origin(),
                    "the server accepted the asset but returned no segment",
                ))
            })?;

        // Properties are stored by segment id, whether or not the segment is registered, so the
        // coverage is in place by the time the asset is.
        if let Some(coverage) = coverage {
            let written = self
                .set_segment_properties(
                    asset_dataset,
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

        registration
            .wait(timeout)
            .await
            .map_err(|err| AssetRegistrationError::new(Some(asset_id.clone()), err.into()))?;

        Ok(asset_id)
    }

    /// Read which segments of `dataset_id` load one of its assets, and the revision that gates
    /// writes to it.
    ///
    /// An asset the server stored no coverage for applies to every segment, and its revision is
    /// `0`.
    pub async fn asset_coverage(
        &self,
        dataset_id: EntryId,
        asset_id: SegmentId,
    ) -> ApiResult<(AssetCoverage, u64)> {
        // No asset dataset means no asset was ever registered, so there is no coverage to read.
        let Some(asset_dataset) = self.asset_dataset(dataset_id).await? else {
            return Ok((AssetCoverage::default(), 0));
        };

        let stored = self.get_segment_properties(asset_dataset, asset_id).await?;

        Ok(stored.map_or_else(
            || (AssetCoverage::default(), 0),
            |(properties, revision)| (AssetCoverage::from_properties(&properties), revision),
        ))
    }

    /// Replace which segments of `dataset_id` load one of its assets, returning the new revision.
    ///
    /// The write replaces every property stored on the asset, and coverage is the only property an
    /// asset carries.
    ///
    /// With `expected_revision` set the write only lands if the stored revision matches, and fails
    /// with `FailedPrecondition` otherwise, leaving the stored coverage untouched. `0` means "only
    /// if the asset has no coverage stored yet". Without it the write overwrites whatever is
    /// stored.
    pub async fn set_asset_coverage(
        &self,
        dataset_id: EntryId,
        asset_id: SegmentId,
        coverage: &AssetCoverage,
        expected_revision: Option<u64>,
    ) -> ApiResult<u64> {
        let Some(asset_dataset) = self.asset_dataset(dataset_id).await? else {
            return Err(ApiError::internal(
                self.origin(),
                "the dataset holds no assets",
            ));
        };

        self.set_segment_properties(
            asset_dataset,
            asset_id,
            &coverage.to_properties(),
            expected_revision,
        )
        .await
    }

    /// Unregister an asset previously registered with [`Self::register_asset`], and wait for the
    /// unregistration to finish.
    ///
    /// The server drops only the layers of the asset it is done registering. `force` drops them
    /// whatever their status, which is the only way to clear an asset whose registration failed.
    ///
    /// Unregistering an asset that isn't registered does nothing.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn unregister_asset(
        &self,
        dataset_id: EntryId,
        asset_segment_id: SegmentId,
        force: bool,
        timeout: std::time::Duration,
    ) -> ApiResult {
        let mut client = self.client().await?;

        // No asset dataset means no asset was ever registered, so there is nothing to drop.
        let Some(asset_dataset) = client
            .read_dataset_entry(dataset_id)
            .await?
            .dataset_details
            .asset_dataset
        else {
            return Ok(());
        };

        let (trace_id, task_ids) = client
            .unregister_from_dataset(asset_dataset, vec![asset_segment_id], vec![], force)
            .await?;

        client.wait_for_tasks(task_ids, trace_id, timeout).await
    }

    /// The asset dataset of `dataset_id`, if it has one.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn asset_dataset(&self, dataset_id: EntryId) -> ApiResult<Option<EntryId>> {
        Ok(self
            .client()
            .await?
            .read_dataset_entry(dataset_id)
            .await?
            .dataset_details
            .asset_dataset)
    }

    /// The assets of `dataset_id` the server is done registering.
    ///
    /// `None` if the dataset has no asset dataset.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn registered_assets(
        &self,
        dataset_id: EntryId,
    ) -> ApiResult<Option<RegisteredAssets>> {
        let Some(asset_dataset) = self.asset_dataset(dataset_id).await? else {
            return Ok(None);
        };

        let mut assets = self.scan_asset_dataset(asset_dataset).await?;
        assets.retain(ManifestSegment::is_registered);
        Ok(Some(RegisteredAssets {
            asset_dataset,
            assets,
        }))
    }

    /// The Sorbet schema of each asset of `dataset_id` the server is done registering, sorted by
    /// asset id.
    ///
    /// Empty if the dataset has no asset dataset.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_asset_schemas(
        &self,
        dataset_id: EntryId,
    ) -> ApiResult<Vec<(SegmentId, ArrowSchema)>> {
        let Some(RegisteredAssets {
            asset_dataset,
            assets,
        }) = self.registered_assets(dataset_id).await?
        else {
            return Ok(Vec::new());
        };

        let schemas = assets.into_iter().map(|asset| async move {
            let schema = self
                .get_segment_schema(asset_dataset, asset.id.clone(), None)
                .await?;
            Ok((asset.id, schema))
        });
        futures::future::try_join_all(schemas).await
    }

    /// The asset dataset of `dataset_id`, creating one if it has none yet.
    ///
    /// Datasets are now always created with an asset dataset, but there could still be
    /// datasets created before.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn ensure_asset_dataset(&self, dataset_id: EntryId) -> ApiResult<EntryId> {
        let mut client = self.client().await?;
        let dataset_details = client.read_dataset_entry(dataset_id).await?.dataset_details;

        if let Some(asset_dataset) = dataset_details.asset_dataset {
            return Ok(asset_dataset);
        }

        // The server attaches the missing asset dataset while handling an update, so send it back
        // the details it already has.
        client
            .update_dataset_entry(dataset_id, dataset_details)
            .await?
            .dataset_details
            .asset_dataset
            .ok_or_else(|| {
                ApiError::internal(
                    self.origin(),
                    "the server did not attach an asset dataset to this dataset",
                )
            })
    }
}
