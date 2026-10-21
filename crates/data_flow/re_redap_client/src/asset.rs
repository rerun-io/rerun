//! Resolve asset coverage and chunk manifests on the data path.

use crate::{ApiError, ApiErrorKind, ApiResult, ConnectionClient};
use arrow::array::RecordBatch;
use re_chunk_index::{RawRrdManifest, RrdManifest};
use re_log_encoding::CodecResult;
use re_log_types::EntryId;
use re_types_core::SegmentId;

/// Each asset carries its own mode and segment list. Recordings hold no asset properties.
/// Assets that hold no `asset` property resolve to the opt-out default and apply to every segment.
pub fn filter_asset_segments(
    origin: &re_uri::Origin,
    asset_segment_ids: &mut Vec<SegmentId>,
    requested_segment: Option<&SegmentId>,
    response: re_protos::cloud::v1alpha1::GetSegmentPropertiesResponse,
) -> ApiResult<()> {
    use re_protos::cloud::v1alpha1::ext::{
        asset_applies_to_segment, read_asset_mode, read_asset_segments,
    };

    for segment in response.segments {
        let id = segment.segment_id.ok_or_else(|| {
            ApiError::deserialization_with_source(
                origin,
                None,
                re_protos::missing_field!(
                    re_protos::cloud::v1alpha1::SegmentProperties,
                    "segment_id"
                ),
                "missing segment id in /GetSegmentProperties response",
            )
        })?;
        let id = SegmentId::try_from(id).map_err(|err| {
            ApiError::deserialization_with_source(origin, None, err, "invalid asset segment id")
        })?;
        let Some(properties) = segment.properties else {
            continue;
        };
        let batch = RecordBatch::try_from(&properties).map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                None,
                err,
                "failed decoding asset properties",
            )
        })?;
        if !asset_applies_to_segment(
            read_asset_mode(&batch, 0),
            requested_segment.map(SegmentId::as_str),
            &read_asset_segments(&batch, 0),
        ) {
            asset_segment_ids.retain(|asset_id| asset_id != &id);
        }
    }
    Ok(())
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
    segment_id: &SegmentId,
) -> Option<AssetSegments> {
    match client
        .get_assets_for_segment(dataset_id, Some(segment_id.clone()))
        .await
    {
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
    use super::*;

    /// Coverage includes opted-in segments and excludes opted-out segments.
    /// Assets without properties apply to every segment, across multiple response batches.
    #[test]
    fn coverage_filters_asset_segments() {
        use re_protos::cloud::v1alpha1::ext::{AssetMode, asset_properties};
        use re_protos::cloud::v1alpha1::{GetSegmentPropertiesResponse, SegmentProperties};

        for (requested, expected) in [
            (Some("listed"), vec!["bare", "included"]),
            (Some("other"), vec!["bare", "excluded"]),
            (None, vec!["bare", "excluded"]),
        ] {
            let mut ids = vec!["bare".into(), "included".into(), "excluded".into()];
            let requested = requested.map(SegmentId::from);
            for (id, mode) in [
                ("included", AssetMode::OptIn),
                ("excluded", AssetMode::OptOut),
            ] {
                filter_asset_segments(
                    &re_uri::Origin::test(),
                    &mut ids,
                    requested.as_ref(),
                    GetSegmentPropertiesResponse {
                        segments: vec![SegmentProperties {
                            segment_id: Some(SegmentId::from(id).into()),
                            properties: Some((&asset_properties(mode, ["listed"])).into()),
                            revision: 1,
                        }],
                    },
                )
                .unwrap();
            }
            assert_eq!(
                ids,
                expected
                    .into_iter()
                    .map(SegmentId::from)
                    .collect::<Vec<_>>()
            );
        }
    }
}
