//! The assets of a dataset.

use std::collections::hash_map;

use ahash::HashMap;
use arrow::array::RecordBatch;
use re_log_types::Timestamp;
use re_protos::cloud::v1alpha1::ext::ScanDatasetManifestDataframe;
use re_types_core::SegmentId;

use crate::{ApiError, ApiResult};

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
}

/// The manifest columns an [`Asset`] is built from.
pub(crate) const ASSET_COLUMNS: [&str; 4] = [
    ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_SIZE_BYTES_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_TIME_NAME,
    ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT_NAME,
];

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

        let rows = itertools::izip!(
            ids.iter_owned(),
            sizes.iter_owned(),
            registration_times.iter_owned(),
            update_times.iter_owned()
        );

        for (id, size, registered_at, updated_at) in rows {
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
                }
            }
        }
    }

    let mut assets: Vec<_> = assets.into_values().collect();
    assets.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(assets)
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
    }

    /// The layers of an asset add up to one asset, no matter which batch of the manifest they
    /// arrive in. The asset is as old as its first layer and as new as its last one.
    #[test]
    fn layers_of_an_asset_are_folded_together() {
        let first = manifest_batch(&[
            ManifestRow {
                asset: "mesh",
                size: Some(100),
                registered_at: 20,
                updated_at: 30,
            },
            ManifestRow {
                asset: "texture",
                size: Some(7),
                registered_at: 40,
                updated_at: 40,
            },
        ]);

        let second = manifest_batch(&[ManifestRow {
            asset: "mesh",
            size: Some(5),
            registered_at: 10,
            updated_at: 25,
        }]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[first, second]).unwrap();

        assert_eq!(
            assets,
            vec![
                Asset {
                    id: SegmentId::new("mesh".to_owned()),
                    size: 105,
                    registered_at: Timestamp::from_nanos_since_epoch(10),
                    last_updated_at: Timestamp::from_nanos_since_epoch(30),
                },
                Asset {
                    id: SegmentId::new("texture".to_owned()),
                    size: 7,
                    registered_at: Timestamp::from_nanos_since_epoch(40),
                    last_updated_at: Timestamp::from_nanos_since_epoch(40),
                },
            ]
        );
    }

    /// A layer that has no size yet counts as empty instead of hiding the asset it belongs to.
    #[test]
    fn a_layer_without_a_size_counts_as_empty() {
        let batch = manifest_batch(&[
            ManifestRow {
                asset: "mesh",
                size: None,
                registered_at: 10,
                updated_at: 10,
            },
            ManifestRow {
                asset: "mesh",
                size: Some(9),
                registered_at: 10,
                updated_at: 10,
            },
        ]);

        let assets = assets_from_manifest(&re_uri::Origin::test(), &[batch]).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].size, 9);
    }

    /// A manifest holding only the columns the server was asked to project.
    fn manifest_batch(rows: &[ManifestRow]) -> RecordBatch {
        let schema = Schema::new_with_metadata(
            vec![
                ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_SIZE_BYTES.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_TIME.arrow_field(),
                ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT.arrow_field(),
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
            ],
            &RecordBatchOptions::default().with_row_count(Some(rows.len())),
        )
        .unwrap()
    }
}
