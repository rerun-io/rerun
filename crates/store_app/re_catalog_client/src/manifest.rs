//! The manifest of a dataset, folded into one [`ManifestSegment`] per segment.

use std::collections::{BTreeSet, hash_map};

use ahash::HashMap;
use arrow::array::RecordBatch;

use re_log_types::{EntryId, Timestamp};
use re_protos::cloud::v1alpha1::ext::{LayerRegistrationStatus, ScanDatasetManifestDataframe};
use re_redap_client::{ApiError, ApiResult};
use re_types_core::{LayerName, SegmentId};

use crate::CatalogClient;

/// One segment of a dataset's manifest, with all its layers folded together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestSegment {
    pub id: SegmentId,

    /// How many bytes the segment takes over all its layers.
    pub size: u64,

    /// When the first layer of the segment was registered.
    pub registered_at: Timestamp,

    /// When any layer of the segment was last touched.
    pub last_updated_at: Timestamp,

    /// Where the segment's data came from, one entry per layer, sorted by layer name.
    pub layers: Vec<ManifestLayer>,

    /// How far the server got registering the segment.
    ///
    /// A registration that failed leaves its segment listed, so a segment being listed does not
    /// mean the server took it.
    pub status: LayerRegistrationStatus,
}

impl ManifestSegment {
    /// Whether the server is done registering the segment, so its data can be read.
    pub fn is_registered(&self) -> bool {
        self.status == LayerRegistrationStatus::Done
    }

    /// Whether the server gave up on the segment, leaving it listed with no data behind it.
    pub fn has_failed(&self) -> bool {
        self.status == LayerRegistrationStatus::Error
    }
}

/// One layer of a [`ManifestSegment`], and where its data is stored.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManifestLayer {
    pub name: LayerName,

    /// The URI the layer was registered from.
    pub storage_url: String,
}

/// The manifest columns a [`ManifestSegment`] is built from.
const MANIFEST_COLUMNS: [&str; 7] = [
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
        re_log::warn_once!("Unknown layer registration status {status:?}, treating it as done");
        LayerRegistrationStatus::Done
    })
}

/// The status the layers of a segment add up to.
///
/// A layer that failed decides the segment's status, since the segment is incomplete either way,
/// and one the server is still working on outranks a finished one.
fn merged_status(
    left: LayerRegistrationStatus,
    right: LayerRegistrationStatus,
) -> LayerRegistrationStatus {
    fn rank(status: LayerRegistrationStatus) -> u8 {
        match status {
            LayerRegistrationStatus::Error => 3,
            LayerRegistrationStatus::Pending => 2,
            LayerRegistrationStatus::Done => 1,
            #[expect(deprecated, reason = "soft deletes are no longer used")]
            LayerRegistrationStatus::Deleted => 0,
        }
    }

    std::cmp::max_by_key(left, right, |status| rank(*status))
}

/// One [`ManifestSegment`] per segment of a dataset's manifest, sorted by segment id.
///
/// The manifest has one row per layer, so the layers of a segment are folded together.
fn segments_from_manifest(
    origin: &re_uri::Origin,
    batches: &[RecordBatch],
) -> ApiResult<Vec<ManifestSegment>> {
    let decode_failed =
        |err| ApiError::deserialization_quiver_from(origin, None, err, "/ScanDatasetManifest");

    let mut segments: HashMap<SegmentId, ManifestSegment> = HashMap::default();

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
            let layer = ManifestLayer {
                name: layer_name,
                storage_url,
            };

            match segments.entry(id.clone()) {
                hash_map::Entry::Vacant(slot) => {
                    slot.insert(ManifestSegment {
                        id,
                        size,
                        registered_at: created_at,
                        last_updated_at,
                        layers: vec![layer],
                        status,
                    });
                }

                hash_map::Entry::Occupied(mut slot) => {
                    // An segment is as old as its first layer and as new as its last one.
                    let segment = slot.get_mut();
                    segment.size += size;
                    segment.registered_at =
                        std::cmp::min_by_key(segment.registered_at, created_at, |time| {
                            time.nanos_since_epoch()
                        });
                    segment.last_updated_at =
                        std::cmp::max_by_key(segment.last_updated_at, last_updated_at, |time| {
                            time.nanos_since_epoch()
                        });
                    segment.layers.push(layer);
                    segment.status = merged_status(segment.status, status);
                }
            }
        }
    }

    let mut segments: Vec<_> = segments.into_values().collect();
    segments.sort_by(|left, right| left.id.cmp(&right.id));
    for segment in &mut segments {
        segment.layers.sort();
    }

    Ok(segments)
}

impl CatalogClient {
    /// Scan a dataset's manifest, one [`ManifestSegment`] per segment, sorted by segment id.
    ///
    /// Only the segments in `segment_ids` if it is `Some`.
    pub(super) async fn scan_manifest(
        &self,
        dataset: EntryId,
        segment_ids: Option<&BTreeSet<SegmentId>>,
    ) -> ApiResult<Vec<ManifestSegment>> {
        let manifest = self
            .client()
            .await?
            .scan_dataset_manifest(dataset, MANIFEST_COLUMNS, segment_ids, None)
            .await?;

        let mut segments = segments_from_manifest(self.origin(), &manifest.batches)?;

        // Servers may ignore the filter hint, so filter locally too.
        if let Some(segment_ids) = segment_ids {
            segments.retain(|segment| segment_ids.contains(&segment.id));
        }

        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{RecordBatchOptions, StringArray, TimestampNanosecondArray, UInt64Array};
    use arrow::datatypes::Schema;
    use std::sync::Arc;

    struct ManifestRow {
        segment: &'static str,
        size: Option<u64>,
        registered_at: i64,
        updated_at: i64,
        layer: &'static str,
        storage_url: &'static str,
        status: LayerRegistrationStatus,
    }

    /// A layer the server is done with, which is what most rows of a manifest look like.
    fn done(
        segment: &'static str,
        size: Option<u64>,
        registered_at: i64,
        updated_at: i64,
        layer: &'static str,
        storage_url: &'static str,
    ) -> ManifestRow {
        ManifestRow {
            segment,
            size,
            registered_at,
            updated_at,
            layer,
            storage_url,
            status: LayerRegistrationStatus::Done,
        }
    }

    /// The layers of a segment add up to one segment, no matter which batch of the manifest they
    /// arrive in. The segment is as old as its first layer and as new as its last one.
    #[test]
    fn layers_of_an_segment_are_folded_together() {
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

        let segments = segments_from_manifest(&re_uri::Origin::test(), &[first, second]).unwrap();

        assert_eq!(
            segments,
            vec![
                ManifestSegment {
                    id: SegmentId::new("mesh".to_owned()),
                    size: 105,
                    registered_at: Timestamp::from_nanos_since_epoch(10),
                    last_updated_at: Timestamp::from_nanos_since_epoch(30),
                    // Sorted by layer name, whichever batch each one arrived in.
                    layers: vec![
                        ManifestLayer {
                            name: LayerName::try_new("base").unwrap(),
                            storage_url: "s3://bucket/mesh-base.rrd".to_owned(),
                        },
                        ManifestLayer {
                            name: LayerName::try_new("high").unwrap(),
                            storage_url: "s3://bucket/mesh-high.rrd".to_owned(),
                        },
                    ],
                    status: LayerRegistrationStatus::Done,
                },
                ManifestSegment {
                    id: SegmentId::new("texture".to_owned()),
                    size: 7,
                    registered_at: Timestamp::from_nanos_since_epoch(40),
                    last_updated_at: Timestamp::from_nanos_since_epoch(40),
                    layers: vec![ManifestLayer {
                        name: LayerName::try_new("base").unwrap(),
                        storage_url: "s3://bucket/texture.rrd".to_owned(),
                    }],
                    status: LayerRegistrationStatus::Done,
                },
            ]
        );
    }

    /// A layer that has no size yet counts as empty instead of hiding the segment it belongs to.
    #[test]
    fn a_layer_without_a_size_counts_as_empty() {
        let batch = manifest_batch(&[
            done("mesh", None, 10, 10, "pending", "s3://bucket/pending.rrd"),
            done("mesh", Some(9), 10, 10, "base", "s3://bucket/mesh.rrd"),
        ]);

        let segments = segments_from_manifest(&re_uri::Origin::test(), &[batch]).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].size, 9);
    }

    /// A registration that failed stays in the manifest, so its segment is listed as failed rather
    /// than as one the server took. A layer that failed decides the whole segment, even when its
    /// other layers went through.
    #[test]
    fn a_layer_that_failed_makes_the_whole_segment_failed() {
        let batch = manifest_batch(&[
            done("mesh", Some(9), 10, 10, "base", "s3://bucket/mesh.rrd"),
            ManifestRow {
                segment: "mesh",
                size: None,
                registered_at: 10,
                updated_at: 10,
                layer: "high",
                storage_url: "s3://bucket/mesh-high.rrd",
                status: LayerRegistrationStatus::Error,
            },
        ]);

        let segments = segments_from_manifest(&re_uri::Origin::test(), &[batch]).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].status, LayerRegistrationStatus::Error);
        assert!(!segments[0].is_registered());
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
                    rows.iter().map(|row| row.segment),
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
