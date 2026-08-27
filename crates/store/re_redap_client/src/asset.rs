//! The assets of a dataset.

use std::collections::hash_map;

use ahash::HashMap;
use arrow::array::RecordBatch;
use re_log_encoding::{CodecResult, RawRrdManifest, RrdManifest};
use re_log_types::{EntryId, Timestamp};
use re_protos::cloud::v1alpha1::ext::{
    DataSource, LayerRegistrationStatus, ScanDatasetManifestDataframe,
};
use re_types_core::SegmentId;

use crate::{ApiError, ApiErrorKind, ApiResult, ConnectionClient};

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
    pub(crate) fn without_asset(error: ApiError) -> Self {
        Self {
            asset_id: None,
            error,
        }
    }

    pub(crate) fn new(asset_id: Option<SegmentId>, error: ApiError) -> Self {
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
    DataSource::new_rrd(asset_uri).map_err(|err| {
        ApiError::invalid_arguments_with_source(origin, None, err, "invalid asset url")
    })
}

/// The manifest columns an [`Asset`] is built from.
pub(crate) const ASSET_COLUMNS: [&str; 5] = [
    ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_SIZE_BYTES_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_TIME_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT_NAME,
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
            LayerRegistrationStatus::Deleted => 0,
        }
    }

    std::cmp::max_by_key(left, right, |status| rank(*status))
}

/// One [`Asset`] per segment of an asset dataset's manifest, sorted by asset id.
///
/// The manifest has one row per layer, so the layers of a segment are folded together.
pub(crate) fn assets_from_manifest(
    origin: &re_uri::Origin,
    batches: &[RecordBatch],
) -> ApiResult<Vec<Asset>> {
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
        let statuses = ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_STATUS
            .extract(batch)
            .map_err(decode_failed)?;

        let rows = itertools::izip!(
            ids.iter_owned(),
            sizes.iter_owned(),
            registration_times.iter_owned(),
            update_times.iter_owned(),
            statuses.iter_owned()
        );

        for (id, size, registered_at, updated_at, status) in rows {
            let status = layer_status(&status);

            // A layer the server dropped is only kept around until it is cleaned up, so it is not
            // part of the asset any more.
            if status == LayerRegistrationStatus::Deleted {
                continue;
            }

            // A layer that is still being registered has no size yet.
            let size = size.unwrap_or(0);
            let created_at = Timestamp::from_nanos_since_epoch(registered_at);
            let last_updated_at = Timestamp::from_nanos_since_epoch(updated_at);

            match assets.entry(id.clone()) {
                hash_map::Entry::Vacant(slot) => {
                    slot.insert(Asset {
                        id,
                        size,
                        registered_at: created_at,
                        last_updated_at,
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
                    asset.status = merged_status(asset.status, status);
                }
            }
        }
    }

    let mut assets: Vec<_> = assets.into_values().collect();
    assets.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(assets)
}

/// The asset dataset of a dataset, and every asset segment within it.
pub(crate) struct AssetSegments {
    pub dataset_id: EntryId,
    pub segment_ids: Vec<SegmentId>,
}

/// The assets registered for a dataset, or `None` when it has none or they could not be fetched.
pub(crate) async fn asset_segments(
    client: &mut ConnectionClient,
    dataset_id: EntryId,
) -> Option<AssetSegments> {
    match client.get_assets_for_segment(dataset_id).await {
        Ok(Some((dataset_id, segment_ids))) => Some(AssetSegments {
            dataset_id,
            segment_ids,
        }),

        Ok(None) => None,

        Err(err) => {
            if !matches!(
                err.kind,
                ApiErrorKind::NotFound
                    | ApiErrorKind::Unimplemented
                    | ApiErrorKind::InvalidArguments
            ) {
                re_log::warn!("Failed to fetch assets: {err}");
            }
            None
        }
    }
}

/// The manifest of one asset, without its recording properties and with its chunks marked for
/// caching.
pub(crate) fn asset_manifest(
    client: &ConnectionClient,
    raw_manifest: RawRrdManifest,
) -> CodecResult<(RawRrdManifest, RrdManifest)> {
    let raw_manifest = raw_manifest.without_recording_properties()?;
    let manifest = RrdManifest::try_new(&raw_manifest)?;

    client.mark_asset_chunks(manifest.col_chunk_ids());

    Ok((raw_manifest, manifest))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{RecordBatchOptions, StringArray, TimestampNanosecondArray, UInt64Array};
    use arrow::datatypes::Schema;

    use super::*;

    struct ManifestRow {
        asset: &'static str,
        size: Option<u64>,
        registered_at: i64,
        updated_at: i64,
        status: LayerRegistrationStatus,
    }

    /// A layer the server is done with, which is what most rows of a manifest look like.
    fn done(
        asset: &'static str,
        size: Option<u64>,
        registered_at: i64,
        updated_at: i64,
    ) -> ManifestRow {
        ManifestRow {
            asset,
            size,
            registered_at,
            updated_at,
            status: LayerRegistrationStatus::Done,
        }
    }

    /// The layers of an asset add up to one asset, no matter which batch of the manifest they
    /// arrive in. The asset is as old as its first layer and as new as its last one.
    #[test]
    fn layers_of_an_asset_are_folded_together() {
        let first = manifest_batch(&[
            done("mesh", Some(100), 20, 30),
            done("texture", Some(7), 40, 40),
        ]);

        let second = manifest_batch(&[done("mesh", Some(5), 10, 25)]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[first, second]).unwrap();

        assert_eq!(
            assets,
            vec![
                Asset {
                    id: SegmentId::new("mesh".to_owned()),
                    size: 105,
                    registered_at: Timestamp::from_nanos_since_epoch(10),
                    last_updated_at: Timestamp::from_nanos_since_epoch(30),
                    status: LayerRegistrationStatus::Done,
                },
                Asset {
                    id: SegmentId::new("texture".to_owned()),
                    size: 7,
                    registered_at: Timestamp::from_nanos_since_epoch(40),
                    last_updated_at: Timestamp::from_nanos_since_epoch(40),
                    status: LayerRegistrationStatus::Done,
                },
            ]
        );
    }

    /// A layer that has no size yet counts as empty instead of hiding the asset it belongs to.
    #[test]
    fn a_layer_without_a_size_counts_as_empty() {
        let batch = manifest_batch(&[done("mesh", None, 10, 10), done("mesh", Some(9), 10, 10)]);

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
            done("mesh", Some(9), 10, 10),
            ManifestRow {
                asset: "mesh",
                size: None,
                registered_at: 10,
                updated_at: 10,
                status: LayerRegistrationStatus::Error,
            },
        ]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[batch]).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].status, LayerRegistrationStatus::Error);
        assert!(!assets[0].is_registered());
    }

    /// A layer the server dropped is not part of its asset any more, and an asset left without a
    /// single layer is not listed at all.
    #[test]
    fn a_dropped_layer_is_left_out() {
        let batch = manifest_batch(&[
            done("mesh", Some(9), 10, 10),
            ManifestRow {
                asset: "dropped",
                size: Some(4),
                registered_at: 10,
                updated_at: 10,
                status: LayerRegistrationStatus::Deleted,
            },
        ]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[batch]).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id, SegmentId::new("mesh".to_owned()));
    }

    /// A manifest holding only the columns the server was asked to project.
    fn manifest_batch(rows: &[ManifestRow]) -> RecordBatch {
        let schema = Schema::new_with_metadata(
            vec![
                ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_SIZE_BYTES.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_TIME.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT.arrow_field(),
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
                    rows.iter().map(|row| row.status.as_str()),
                )),
            ],
            &RecordBatchOptions::default().with_row_count(Some(rows.len())),
        )
        .unwrap()
    }
}
