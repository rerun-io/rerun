//! Catalog assets and their registration lifecycle.

use std::collections::hash_map;

use ahash::HashMap;
use arrow::array::RecordBatch;

use re_log_types::{EntryId, Timestamp};
use re_protos::cloud::v1alpha1::ext::{
    AssetMode, DataSource, LayerRegistrationStatus, ScanDatasetManifestDataframe, asset_properties,
    read_asset_mode, read_asset_segments,
};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::{ApiError, ApiErrorKind, ApiResult};
use re_types_core::{LayerName, SegmentId};

use crate::CatalogClient;

/// One asset of a dataset, as it stands in its asset dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub id: SegmentId,

    /// How many bytes the asset takes over all its layers.
    pub size: u64,

    /// When the first layer of the asset was registered.
    pub registered_at: Timestamp,

    /// When any layer of the asset was last touched.
    pub last_updated_at: Timestamp,

    /// Where the asset's data came from, one entry per layer, sorted by layer name.
    pub layers: Vec<AssetLayer>,

    /// How far the server got registering the asset.
    ///
    /// A registration that failed leaves its asset listed, so an asset being listed does not mean
    /// the server took it.
    pub status: LayerRegistrationStatus,
}

impl Asset {
    /// Whether the server is done registering the asset, so its data can be read.
    pub fn is_registered(&self) -> bool {
        self.status == LayerRegistrationStatus::Done
    }

    /// Whether the server gave up on the asset, leaving it listed with no data behind it.
    pub fn has_failed(&self) -> bool {
        self.status == LayerRegistrationStatus::Error
    }
}

/// One layer of an [`Asset`], and where its data is stored.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetLayer {
    pub name: LayerName,

    /// The URI the layer was registered from.
    pub storage_url: String,
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

    pub error: ApiError,
}

impl AssetRegistrationError {
    /// A failure from before the server started registering an asset, so nothing is registered.
    fn without_asset(error: ApiError) -> Self {
        Self {
            asset_id: None,
            error,
        }
    }

    fn new(asset_id: Option<SegmentId>, error: ApiError) -> Self {
        Self { asset_id, error }
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

/// The manifest columns an [`Asset`] is built from.
const ASSET_COLUMNS: [&str; 7] = [
    ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_SIZE_BYTES_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_TIME_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_STORAGE_URL_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_STATUS_NAME,
];

/// The status of one layer, as the server reported it.
///
/// A status the viewer doesn't know comes from a newer server, so the layer reads as done.
fn layer_status(status: &str) -> LayerRegistrationStatus {
    status.parse().unwrap_or_else(|_| {
        re_log::warn_once!("Unknown asset registration status {status:?}, treating it as done");
        LayerRegistrationStatus::Done
    })
}

/// The status the layers of an asset add up to.
///
/// A layer that failed decides the asset's status, since the asset is incomplete either way, and
/// one the server is still working on outranks a finished one.
fn merged_status(
    left: LayerRegistrationStatus,
    right: LayerRegistrationStatus,
) -> LayerRegistrationStatus {
    fn rank(status: LayerRegistrationStatus) -> u8 {
        match status {
            LayerRegistrationStatus::Error => 3,
            LayerRegistrationStatus::Pending => 2,
            LayerRegistrationStatus::Done => 1,
            #[expect(
                deprecated,
                reason = "asset datasets postdate soft deletes; never reported"
            )]
            LayerRegistrationStatus::Deleted => 0,
        }
    }

    std::cmp::max_by_key(left, right, |status| rank(*status))
}

/// One [`Asset`] per segment of an asset dataset's manifest, sorted by asset id.
///
/// The manifest has one row per layer, so the layers of a segment are folded together.
fn assets_from_manifest(origin: &re_uri::Origin, batches: &[RecordBatch]) -> ApiResult<Vec<Asset>> {
    let decode_failed =
        |err| ApiError::deserialization_quiver_from(origin, None, err, "/ScanDatasetManifest");

    let mut assets: HashMap<SegmentId, Asset> = HashMap::default();

    for batch in batches {
        let ids = ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID
            .extract(batch)
            .map_err(decode_failed)?;
        let sizes = ScanDatasetManifestDataframe::COLUMN_RERUN_SIZE_BYTES
            .extract(batch)
            .map_err(decode_failed)?;
        let registration_times = ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_TIME
            .extract(batch)
            .map_err(decode_failed)?;
        let update_times = ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT
            .extract(batch)
            .map_err(decode_failed)?;
        let layer_names = ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME
            .extract(batch)
            .map_err(decode_failed)?;
        let storage_urls = ScanDatasetManifestDataframe::COLUMN_RERUN_STORAGE_URL
            .extract(batch)
            .map_err(decode_failed)?;
        let statuses = ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_STATUS
            .extract(batch)
            .map_err(decode_failed)?;

        let rows = itertools::izip!(
            ids.iter_owned(),
            sizes.iter_owned(),
            registration_times.iter_owned(),
            update_times.iter_owned(),
            layer_names.iter_owned(),
            storage_urls.iter_owned(),
            statuses.iter_owned()
        );

        for (id, size, registered_at, updated_at, layer_name, storage_url, status) in rows {
            let status = layer_status(&status);

            // A layer that is still being registered has no size yet.
            let size = size.unwrap_or(0);
            let created_at = Timestamp::from_nanos_since_epoch(registered_at);
            let last_updated_at = Timestamp::from_nanos_since_epoch(updated_at);
            let layer = AssetLayer {
                name: layer_name,
                storage_url,
            };

            match assets.entry(id.clone()) {
                hash_map::Entry::Vacant(slot) => {
                    slot.insert(Asset {
                        id,
                        size,
                        registered_at: created_at,
                        last_updated_at,
                        layers: vec![layer],
                        status,
                    });
                }

                hash_map::Entry::Occupied(mut slot) => {
                    // An asset is as old as its first layer and as new as its last one.
                    let asset = slot.get_mut();
                    asset.size += size;
                    asset.registered_at =
                        std::cmp::min_by_key(asset.registered_at, created_at, |time| {
                            time.nanos_since_epoch()
                        });
                    asset.last_updated_at =
                        std::cmp::max_by_key(asset.last_updated_at, last_updated_at, |time| {
                            time.nanos_since_epoch()
                        });
                    asset.layers.push(layer);
                    asset.status = merged_status(asset.status, status);
                }
            }
        }
    }

    let mut assets: Vec<_> = assets.into_values().collect();
    assets.sort_by(|left, right| left.id.cmp(&right.id));
    for asset in &mut assets {
        asset.layers.sort();
    }

    Ok(assets)
}

impl CatalogClient {
    /// Get the asset dataset that applies to a dataset, and the asset segments within it.
    ///
    /// `segment_id` narrows the result to the assets that apply to that segment, honoring each
    /// asset's mode and segment list. When it is `None`, the per-asset defaults apply with no
    /// segment-specific opt-out/opt-in overrides.
    ///
    /// Returns `None` if the dataset has no asset dataset, which means no assets were ever
    /// registered for it.
    pub async fn get_assets_for_segment(
        &self,
        dataset_id: EntryId,
        segment_id: Option<SegmentId>,
    ) -> ApiResult<Option<(EntryId, Vec<SegmentId>)>> {
        self.client()
            .await?
            .get_assets_for_segment(dataset_id, segment_id)
            .await
    }

    /// Scan an asset dataset's manifest, one [`Asset`] per asset, sorted by asset id.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn scan_asset_dataset(&self, asset_dataset: EntryId) -> ApiResult<Vec<Asset>> {
        let batches = self
            .client()
            .await?
            .scan_dataset_manifest(asset_dataset, ASSET_COLUMNS)
            .await?;

        assets_from_manifest(self.origin(), &batches)
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
            .map(|descriptor| descriptor.segment_id.clone());

        let registered = registration
            .wait(timeout)
            .await
            .map_err(|err| AssetRegistrationError::new(asset_id.clone(), err))?
            .into_iter()
            .next();

        let registered = registered.ok_or_else(|| {
            AssetRegistrationError::new(
                asset_id,
                ApiError::internal(
                    self.origin(),
                    "the server registered the asset but returned no segment",
                ),
            )
        })?;

        // The asset only exists once the server registered it, so its coverage is written after
        // the fact. Until this lands the asset applies to every segment.
        if let Some(coverage) = coverage {
            let written = self
                .set_segment_properties(
                    asset_dataset,
                    registered.clone(),
                    &coverage.to_properties(),
                    None,
                )
                .await;

            match written {
                Ok(_) => {}

                // A server without segment properties loads every asset into every segment.
                Err(err) if err.kind == ApiErrorKind::Unimplemented => {
                    re_log::debug_once!("The server does not store asset coverage");
                }

                Err(err) => return Err(AssetRegistrationError::new(Some(registered), err)),
            }
        }

        Ok(registered)
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

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{RecordBatchOptions, StringArray, TimestampNanosecondArray, UInt64Array};
    use arrow::datatypes::Schema;
    use std::sync::Arc;

    struct ManifestRow {
        asset: &'static str,
        size: Option<u64>,
        registered_at: i64,
        updated_at: i64,
        layer: &'static str,
        storage_url: &'static str,
        status: LayerRegistrationStatus,
    }

    /// A layer the server is done with, which is what most rows of a manifest look like.
    fn done(
        asset: &'static str,
        size: Option<u64>,
        registered_at: i64,
        updated_at: i64,
        layer: &'static str,
        storage_url: &'static str,
    ) -> ManifestRow {
        ManifestRow {
            asset,
            size,
            registered_at,
            updated_at,
            layer,
            storage_url,
            status: LayerRegistrationStatus::Done,
        }
    }

    /// The layers of an asset add up to one asset, no matter which batch of the manifest they
    /// arrive in. The asset is as old as its first layer and as new as its last one.
    #[test]
    fn layers_of_an_asset_are_folded_together() {
        let first = manifest_batch(&[
            done(
                "mesh",
                Some(100),
                20,
                30,
                "high",
                "s3://bucket/mesh-high.rrd",
            ),
            done(
                "texture",
                Some(7),
                40,
                40,
                "base",
                "s3://bucket/texture.rrd",
            ),
        ]);

        let second = manifest_batch(&[done(
            "mesh",
            Some(5),
            10,
            25,
            "base",
            "s3://bucket/mesh-base.rrd",
        )]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[first, second]).unwrap();

        assert_eq!(
            assets,
            vec![
                Asset {
                    id: SegmentId::new("mesh".to_owned()),
                    size: 105,
                    registered_at: Timestamp::from_nanos_since_epoch(10),
                    last_updated_at: Timestamp::from_nanos_since_epoch(30),
                    // Sorted by layer name, whichever batch each one arrived in.
                    layers: vec![
                        AssetLayer {
                            name: LayerName::try_new("base").unwrap(),
                            storage_url: "s3://bucket/mesh-base.rrd".to_owned(),
                        },
                        AssetLayer {
                            name: LayerName::try_new("high").unwrap(),
                            storage_url: "s3://bucket/mesh-high.rrd".to_owned(),
                        },
                    ],
                    status: LayerRegistrationStatus::Done,
                },
                Asset {
                    id: SegmentId::new("texture".to_owned()),
                    size: 7,
                    registered_at: Timestamp::from_nanos_since_epoch(40),
                    last_updated_at: Timestamp::from_nanos_since_epoch(40),
                    layers: vec![AssetLayer {
                        name: LayerName::try_new("base").unwrap(),
                        storage_url: "s3://bucket/texture.rrd".to_owned(),
                    }],
                    status: LayerRegistrationStatus::Done,
                },
            ]
        );
    }

    /// A layer that has no size yet counts as empty instead of hiding the asset it belongs to.
    #[test]
    fn a_layer_without_a_size_counts_as_empty() {
        let batch = manifest_batch(&[
            done("mesh", None, 10, 10, "pending", "s3://bucket/pending.rrd"),
            done("mesh", Some(9), 10, 10, "base", "s3://bucket/mesh.rrd"),
        ]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[batch]).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].size, 9);
    }

    /// A registration that failed stays in the manifest, so its asset is listed as failed rather
    /// than as one the server took. A layer that failed decides the whole asset, even when its
    /// other layers went through.
    #[test]
    fn a_layer_that_failed_makes_the_whole_asset_failed() {
        let batch = manifest_batch(&[
            done("mesh", Some(9), 10, 10, "base", "s3://bucket/mesh.rrd"),
            ManifestRow {
                asset: "mesh",
                size: None,
                registered_at: 10,
                updated_at: 10,
                layer: "high",
                storage_url: "s3://bucket/mesh-high.rrd",
                status: LayerRegistrationStatus::Error,
            },
        ]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[batch]).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].status, LayerRegistrationStatus::Error);
        assert!(!assets[0].is_registered());
    }

    /// A manifest holding only the columns the server was asked to project.
    fn manifest_batch(rows: &[ManifestRow]) -> RecordBatch {
        let schema = Schema::new_with_metadata(
            vec![
                ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_SIZE_BYTES.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_TIME.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_STORAGE_URL.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_STATUS.arrow_field(),
            ],
            Default::default(),
        );

        RecordBatch::try_new_with_options(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from_iter_values(
                    rows.iter().map(|row| row.asset),
                )),
                Arc::new(rows.iter().map(|row| row.size).collect::<UInt64Array>()),
                Arc::new(TimestampNanosecondArray::from_iter_values(
                    rows.iter().map(|row| row.registered_at),
                )),
                Arc::new(TimestampNanosecondArray::from_iter_values(
                    rows.iter().map(|row| row.updated_at),
                )),
                Arc::new(StringArray::from_iter_values(
                    rows.iter().map(|row| row.layer),
                )),
                Arc::new(StringArray::from_iter_values(
                    rows.iter().map(|row| row.storage_url),
                )),
                Arc::new(StringArray::from_iter_values(
                    rows.iter().map(|row| row.status.as_str()),
                )),
            ],
            &RecordBatchOptions::default().with_row_count(Some(rows.len())),
        )
        .unwrap()
    }
}
