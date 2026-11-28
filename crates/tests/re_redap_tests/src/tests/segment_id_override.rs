//! Registering data sources under a segment id that is not their recording id.

use std::collections::{BTreeMap, BTreeSet};

use itertools::{Itertools as _, izip};
use re_chunk_index::RawRrdManifest;
use re_log_types::{TimeInt, TimeType};
use re_protos::cloud::v1alpha1::ext::{
    ChunkCandidatesDataframe, DataSource, DatasetEntry, FillMode, FindChunksRequest,
    SegmentSelection,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    ClientMeta, GetRrdManifestRequest, ReadDatasetEntryRequest, RegisterWithDatasetRequest,
};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::protocol_level::ProtocolLevel;
use re_sdk::external::re_log_encoding::ToApplication as _;
use re_types_core::{ChunkId, LayerName, SegmentId};

use super::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};
use super::find_chunks::{
    find_chunks_batches, one_chunk_per_frame_ids, range_request, samples_request,
};

const TUID_BASE_A: u64 = 700;
const TUID_BASE_B: u64 = 701;
const TUID_PATCH: u64 = 702;

/// Two base recordings registered under other segment ids, plus one patch recording registered as
/// a layer of both segments.
///
/// Every recording has one entity with one temporal chunk per frame at `start + {10, 20, 30, 40}`.
/// The base recordings start at 1000 and 2000, the patch at 1000.
pub async fn register_with_segment_id_override(service: impl RerunCloudService) {
    let dataset_name = "dataset_segment_id_override";
    let segment_x = SegmentId::from("segment_x");
    let segment_y = SegmentId::from("segment_y");
    let base = LayerName::base();
    let patch = LayerName::try_new("patch").expect("valid layer name");

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        TUID_BASE_A,
        [
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "recording_a",
                &["my/entity"],
                1000,
                TimeType::Sequence,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "recording_b",
                &["my/entity"],
                2000,
                TimeType::Sequence,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "recording_patch",
                &["patch/entity"],
                1000,
                TimeType::Sequence,
            )
            .layer_name("patch"),
        ],
    );
    let [base_a, base_b, patch_source] = <[_; 3]>::try_from(data_sources_def.to_data_sources_ext())
        .unwrap_or_else(|_| panic!("three data sources"));

    let data_sources = vec![
        base_a.with_segment_id_override(segment_x.clone()),
        base_b.with_segment_id_override(segment_y.clone()),
        patch_source
            .clone()
            .with_segment_id_override(segment_x.clone()),
        patch_source.with_segment_id_override(segment_y.clone()),
    ];

    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(
            dataset_name,
            data_sources.into_iter().map(Into::into).collect(),
        )
        .await;

    assert_eq!(
        required_client_protocol_level(&service, dataset_name).await,
        ProtocolLevel::CHUNK_SEGMENT_FROM_REQUEST,
        "a segment id override must raise the level the dataset requires of its clients"
    );

    let all_ids = |tuid_prefix| {
        let (temporal, static_id) = one_chunk_per_frame_ids(tuid_prefix);
        std::iter::chain(temporal, [static_id]).collect::<BTreeSet<_>>()
    };

    // Every layer is registered under its overridden segment, and no segment is named after a
    // recording id.
    let expected = BTreeMap::from([
        ((segment_x.clone(), base.clone()), all_ids(TUID_BASE_A)),
        ((segment_y.clone(), base.clone()), all_ids(TUID_BASE_B)),
        ((segment_x.clone(), patch.clone()), all_ids(TUID_PATCH)),
        ((segment_y.clone(), patch.clone()), all_ids(TUID_PATCH)),
    ]);
    let actual = find_chunks_per_segment_layer(
        &service,
        dataset_name,
        range_request(
            "frame_nr",
            None,
            None,
            FillMode::None,
            SegmentSelection::All,
        ),
    )
    .await;
    assert_eq!(
        actual, expected,
        "each layer must be in its overridden segment"
    );

    // The patch's chunks are in both segments, so a sample time of one segment must not select
    // them in the other.
    let (patch_temporal, patch_static) = one_chunk_per_frame_ids(TUID_PATCH);
    let (base_a_temporal, base_a_static) = one_chunk_per_frame_ids(TUID_BASE_A);
    let (_, base_b_static) = one_chunk_per_frame_ids(TUID_BASE_B);
    let samples = BTreeMap::from([
        (
            segment_x.clone(),
            BTreeSet::from([TimeInt::new_temporal(1010)]),
        ),
        (
            segment_y.clone(),
            BTreeSet::from([TimeInt::new_temporal(1030)]),
        ),
    ]);
    let expected = BTreeMap::from([
        (
            (segment_x.clone(), base.clone()),
            BTreeSet::from([base_a_temporal[0], base_a_static]),
        ),
        (
            (segment_y.clone(), base.clone()),
            BTreeSet::from([base_b_static]),
        ),
        (
            (segment_x.clone(), patch.clone()),
            BTreeSet::from([patch_temporal[0], patch_static]),
        ),
        (
            (segment_y.clone(), patch.clone()),
            BTreeSet::from([patch_temporal[2], patch_static]),
        ),
    ]);
    let actual = find_chunks_per_segment_layer(
        &service,
        dataset_name,
        samples_request("frame_nr", samples, FillMode::None),
    )
    .await;
    assert_eq!(
        actual, expected,
        "Samples must select a shared layer's chunks per segment"
    );

    for (segment_id, base_tuid) in [(&segment_x, TUID_BASE_A), (&segment_y, TUID_BASE_B)] {
        let expected: BTreeSet<ChunkId> = all_ids(base_tuid)
            .union(&all_ids(TUID_PATCH))
            .copied()
            .collect();

        let manifest = rrd_manifest(&service, dataset_name, segment_id).await;

        let chunk_ids: BTreeSet<ChunkId> = manifest
            .col_chunk_id()
            .expect("chunk_id column")
            .iter_owned()
            .collect();
        assert_eq!(
            chunk_ids, expected,
            "the manifest of {segment_id} must hold its base and patch layers"
        );

        // Clients name each chunk's segment from this column when a server provides it.
        if manifest
            .data
            .schema()
            .column_with_name(RawRrdManifest::COLUMN_CHUNK_PARTITION_ID.name)
            .is_some()
        {
            let segment_ids = RawRrdManifest::COLUMN_CHUNK_PARTITION_ID
                .extract(&manifest.data)
                .expect("chunk_partition_id column")
                .iter_owned()
                .unique()
                .collect_vec();
            assert_eq!(
                segment_ids,
                vec![segment_id.clone()],
                "the manifest of {segment_id} must name it for every chunk"
            );
        }
    }
}

/// Registering without a segment id override leaves the level a dataset requires of its clients
/// alone, and one override raises it for good: later registrations without one keep it.
pub async fn segment_id_override_raises_required_client_protocol_level(
    service: impl RerunCloudService,
) {
    let dataset_name = "dataset_segment_id_override_protocol_level";
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        720,
        [
            LayerDefinition::simple("recording_plain", &["my/entity"]),
            LayerDefinition::simple("recording_overridden", &["my/entity"]),
            LayerDefinition::simple("recording_later", &["my/entity"]),
        ],
    );
    let [plain, overridden, later] = <[_; 3]>::try_from(data_sources_def.to_data_sources_ext())
        .unwrap_or_else(|_| panic!("three data sources"));

    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, vec![plain.into()])
        .await;
    assert_eq!(
        required_client_protocol_level(&service, dataset_name).await,
        ProtocolLevel::BASELINE,
        "a registration without segment id override must not raise the level"
    );

    service
        .register_with_dataset_name_blocking(
            dataset_name,
            vec![
                overridden
                    .with_segment_id_override("my_segment".into())
                    .into(),
            ],
        )
        .await;
    assert_eq!(
        required_client_protocol_level(&service, dataset_name).await,
        ProtocolLevel::CHUNK_SEGMENT_FROM_REQUEST,
        "a segment id override must raise the level"
    );

    service
        .register_with_dataset_name_blocking(dataset_name, vec![later.into()])
        .await;
    assert_eq!(
        required_client_protocol_level(&service, dataset_name).await,
        ProtocolLevel::CHUNK_SEGMENT_FROM_REQUEST,
        "a later registration without segment id override must not lower the level"
    );
}

async fn required_client_protocol_level(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> ProtocolLevel {
    let dataset: DatasetEntry = service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {})
                .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap_or_else(|err| panic!("read_dataset_entry must succeed: {err}"))
        .into_inner()
        .dataset
        .expect("a dataset entry")
        .try_into()
        .expect("a valid dataset entry");
    dataset.required_client_protocol_level
}

async fn find_chunks_per_segment_layer(
    service: &impl RerunCloudService,
    dataset_name: &str,
    request: FindChunksRequest,
) -> BTreeMap<(SegmentId, LayerName), BTreeSet<ChunkId>> {
    let batches = find_chunks_batches(service, dataset_name, request).await;

    let mut per_segment_layer: BTreeMap<(SegmentId, LayerName), BTreeSet<ChunkId>> =
        BTreeMap::new();
    for batch in &batches {
        let ids = ChunkCandidatesDataframe::COLUMN_CHUNK_ID
            .extract(batch)
            .expect("bad chunk_id column");
        let segments = ChunkCandidatesDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(batch)
            .expect("bad chunk_segment_id column");
        let layers = ChunkCandidatesDataframe::COLUMN_RERUN_SEGMENT_LAYER
            .extract(batch)
            .expect("bad rerun_segment_layer column");

        for (id, segment, layer) in
            izip!(ids.iter_owned(), segments.iter_owned(), layers.iter_owned())
        {
            per_segment_layer
                .entry((segment, layer))
                .or_default()
                .insert(id);
        }
    }
    per_segment_layer
}

/// The manifest of `segment_id`, merged over its layers.
pub async fn rrd_manifest(
    service: &impl RerunCloudService,
    dataset_name: &str,
    segment_id: &SegmentId,
) -> RawRrdManifest {
    use futures::StreamExt as _;

    let parts: Vec<RawRrdManifest> = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some(segment_id.clone().into()),
                generate_direct_urls: false,
                client_meta: Some(ClientMeta::current()),
            })
            .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap_or_else(|err| panic!("get_rrd_manifest must succeed for {segment_id}: {err}"))
        .into_inner()
        .map(|resp| {
            resp.expect("get_rrd_manifest stream must not error")
                .rrd_manifest
                .expect("an inline manifest")
                .to_application(())
                .expect("a valid manifest")
        })
        .collect()
        .await;

    let store_id = parts
        .first()
        .expect("at least one manifest part")
        .store_id
        .clone();
    RawRrdManifest::merge(store_id, parts).expect("mergeable manifest parts")
}

/// The OSS server registers every store of the dataset's kind in a file, so it refuses to put
/// several of them under one segment id.
pub async fn register_multi_store_file_with_segment_id_override(
    service: impl RerunCloudService,
) -> anyhow::Result<()> {
    let dataset_name = "dataset_multi_store_segment_id_override";
    let dir = tempfile::tempdir()?;
    let (path, _) = crate::create_recording_with_shared_recording_id_in(
        710,
        "shared_recording",
        &["app_one", "app_two"],
        dir.path(),
    )?;
    let url = url::Url::from_file_path(&path)
        .map_err(|()| anyhow::anyhow!("invalid file path: {}", path.display()))?;

    service.create_dataset_entry_with_name(dataset_name).await;

    let data_source =
        DataSource::new_rrd_url(url, None).with_segment_id_override("my_segment".into());
    let status = service
        .register_with_dataset(
            tonic::Request::new(RegisterWithDatasetRequest {
                data_sources: vec![data_source.into()],
                on_duplicate: re_protos::common::v1alpha1::IfDuplicateBehavior::Error as i32,
            })
            .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .expect_err("several stores cannot share one segment id");
    assert_eq!(status.code(), tonic::Code::InvalidArgument, "{status}");

    Ok(())
}
