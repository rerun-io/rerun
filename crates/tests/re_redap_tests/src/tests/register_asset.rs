#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{BinaryArray, RecordBatch};
use futures::TryStreamExt as _;
use re_log_types::EntityPath;
use re_protos::cloud::v1alpha1::ext::{
    DataSource as DataSourceExt, DatasetDetails, QueryTasksDataframe,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    DataSource, DeleteEntryRequest, EntryKind, GetAssetsForSegmentRequest, ReadDatasetEntryRequest,
    RegisterWithDatasetRequest,
};
use re_protos::common::v1alpha1::ext::DatasetKind;
use re_protos::common::v1alpha1::{IfDuplicateBehavior, SegmentId};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_sdk_types::AnyValues;
use re_types_core::AsComponents;
use url::Url;

use crate::{
    TempPath, TuidPrefix, create_blueprint_with_static_components, create_minimal_static_recording,
    create_recording_with_static_components,
};

use super::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
    register_and_wait,
};

async fn asset_dataset_id(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> re_log_types::EntryId {
    let dataset_details: DatasetDetails = service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {})
                .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap()
        .into_inner()
        .dataset
        .unwrap()
        .dataset_details
        .unwrap()
        .try_into()
        .unwrap();

    dataset_details
        .asset_dataset
        .expect("dataset should have an asset dataset")
}

/// Resolve a dataset entry's name from its id. Registration and manifest scans are addressed by
/// entry name, so tests targeting an asset or blueprint dataset resolve its name first.
async fn dataset_entry_name(
    service: &impl RerunCloudService,
    entry_id: re_log_types::EntryId,
) -> String {
    service
        .read_dataset_entry(tonic::Request::new(ReadDatasetEntryRequest {}).with_entry_id(entry_id))
        .await
        .unwrap()
        .into_inner()
        .dataset
        .unwrap()
        .details
        .unwrap()
        .name
        .unwrap()
}

async fn asset_dataset_name(service: &impl RerunCloudService, dataset_name: &str) -> String {
    let asset_dataset_id = asset_dataset_id(service, dataset_name).await;
    dataset_entry_name(service, asset_dataset_id).await
}

/// `GetAssetsForSegment` returns the dataset's asset dataset and the assets registered into it.
pub async fn get_assets_for_segment_returns_registered_assets(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    // A normal segment in the main dataset, alongside the asset dataset.
    let main_segments = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("main_segment", &["my/entity"])],
    );
    service
        .register_with_dataset_name_blocking(dataset_name, main_segments.to_data_sources())
        .await;

    // An asset in the asset dataset. A separate tuid prefix avoids chunk-id collisions.
    // Assets must be static-only, so we register a static component rather than a temporal recording.
    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;
    let asset = DataSourcesDefinition::new_with_tuid_prefix(
        100,
        [LayerDefinition::static_components(
            "asset_segment",
            [(
                EntityPath::from("robot/urdf"),
                Box::new(re_sdk_types::archetypes::Points3D::new([(0.0, 0.0, 0.0)]))
                    as Box<dyn AsComponents>,
            )],
        )],
    );
    service
        .register_with_dataset_name_blocking(&asset_dataset_name, asset.to_data_sources())
        .await;

    let responses: Vec<_> = service
        .get_assets_for_segment(
            tonic::Request::new(GetAssetsForSegmentRequest {})
                .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .expect("get_assets_for_segment should succeed")
        .into_inner()
        .try_collect()
        .await
        .expect("get_assets_for_segment stream should succeed");

    let expected_assets_entry = Some(asset_dataset_id(&service, dataset_name).await.into());
    for assets in &responses {
        assert_eq!(
            assets.assets_entry, expected_assets_entry,
            "every response should carry the dataset's asset dataset"
        );
    }

    let asset_segment_ids: Vec<_> = responses
        .into_iter()
        .flat_map(|assets| assets.asset_segment_ids)
        .collect();
    assert_eq!(
        asset_segment_ids,
        vec![SegmentId::from("asset_segment")],
        "should return the registered asset's segment"
    );
}

/// Assets can only be queried on recording datasets, so asking a blueprint or asset dataset for
/// assets is rejected.
pub async fn get_assets_for_segment_rejects_non_recording_dataset(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    let dataset = service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset = dataset
        .dataset_details
        .asset_dataset
        .expect("recording datasets should get an implicit asset dataset");
    let blueprint_dataset = dataset
        .dataset_details
        .blueprint_dataset
        .expect("recording datasets should get an implicit blueprint dataset");

    for non_recording in [asset_dataset, blueprint_dataset] {
        let Err(err) = service
            .get_assets_for_segment(
                tonic::Request::new(GetAssetsForSegmentRequest {}).with_entry_id(non_recording),
            )
            .await
        else {
            panic!("querying assets on a non-recording dataset should fail");
        };
        assert_eq!(
            err.code(),
            tonic::Code::InvalidArgument,
            "unexpected status: {err}"
        );
    }
}

/// Creating a dataset also creates an asset dataset of the right kind, and deleting the dataset
/// deletes the asset dataset along with it, since their lifecycle is tied.
pub async fn deleting_dataset_deletes_asset_dataset(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    let dataset = service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset_id = asset_dataset_id(&service, dataset_name).await;

    let asset_details = service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {}).with_entry_id(asset_dataset_id),
        )
        .await
        .expect("asset dataset should exist before deleting the dataset")
        .into_inner()
        .dataset
        .unwrap()
        .details
        .unwrap();
    assert_eq!(
        asset_details.entry_kind,
        EntryKind::AssetDataset as i32,
        "the asset dataset should have kind AssetDataset"
    );

    service
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            id: Some(dataset.details.id.into()),
        }))
        .await
        .expect("failed to delete dataset entry");

    let asset_status = service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {}).with_entry_id(asset_dataset_id),
        )
        .await
        .unwrap_err();
    assert_eq!(
        asset_status.code(),
        tonic::Code::NotFound,
        "the asset dataset should be deleted with its dataset, got: {asset_status:?}"
    );
}

/// Register an RRD into the asset dataset, returning the gRPC result without waiting for tasks.
async fn try_register_into_asset_dataset(
    service: &impl RerunCloudService,
    asset_dataset_name: &str,
    data_sources: Vec<DataSource>,
) -> tonic::Result<()> {
    service
        .register_with_dataset(
            tonic::Request::new(RegisterWithDatasetRequest {
                data_sources,
                on_duplicate: IfDuplicateBehavior::Error as i32,
            })
            .with_entry_name(entry_name(asset_dataset_name)),
        )
        .await
        .map(|_| ())
}

fn rrd_data_source(path: &TempPath) -> DataSource {
    let url = Url::from_file_path(path.as_path()).expect("valid file path");
    DataSourceExt::new_rrd_url(url).into()
}

/// Assert that at least one registration task failed with a message containing `expected_substring`.
fn assert_task_failed(task_results: &[RecordBatch], expected_substring: &str) {
    for batch in task_results {
        let statuses = QueryTasksDataframe::COLUMN_EXEC_STATUS
            .extract(batch)
            .expect("valid exec_status column");
        let msgs = QueryTasksDataframe::COLUMN_MSGS
            .extract(batch)
            .expect("valid msgs column");

        for (status, msg) in std::iter::zip(&statuses, &msgs) {
            if status != "success" {
                let msg = msg.unwrap_or_default();
                assert!(
                    msg.to_lowercase()
                        .contains(&expected_substring.to_lowercase()),
                    "task failed but message {msg:?} does not contain {expected_substring:?}"
                );
                return;
            }
        }
    }
    panic!("expected at least one failed task, but all tasks succeeded");
}

/// An asset dataset only holds static data, so registering a temporal recording is rejected.
///
/// Both servers handle this the same way: the registration request is accepted, then its task
/// fails because the data is temporal.
pub async fn asset_dataset_rejects_temporal_recording(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;

    let temporal = DataSourcesDefinition::new_with_tuid_prefix(
        100,
        [LayerDefinition::simple("temporal_segment", &["my/entity"])],
    );

    let request = tonic::Request::new(RegisterWithDatasetRequest {
        data_sources: temporal.to_data_sources(),
        on_duplicate: IfDuplicateBehavior::Error as i32,
    })
    .with_entry_name(entry_name(&asset_dataset_name));

    let task_results = register_and_wait(&service, request).await;
    assert_task_failed(&task_results, "asset datasets only accept static chunks");
}

/// An asset dataset rejects registration once it already holds the maximum number of segments.
///
/// Both servers enforce this synchronously, returning `FailedPrecondition`.
pub async fn asset_dataset_enforces_segment_limit(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;

    let max_segments = DatasetKind::Asset
        .limits()
        .max_segment_count
        .expect("asset datasets have a segment limit");

    // Hold the temp recordings alive for the duration of the test.
    let mut recordings = Vec::new();

    for i in 0..max_segments {
        let path = create_minimal_static_recording(100 + i, &format!("asset_{i}")).unwrap();
        // Wait for completion so the segment is committed before the next registration's count check.
        service
            .register_with_dataset_name_blocking(&asset_dataset_name, vec![rrd_data_source(&path)])
            .await;
        recordings.push(path);
    }

    let overflow = create_minimal_static_recording(999, "asset_overflow").unwrap();
    let status = try_register_into_asset_dataset(
        &service,
        &asset_dataset_name,
        vec![rrd_data_source(&overflow)],
    )
    .await
    .expect_err("registering past the segment limit should fail");
    recordings.push(overflow);

    assert_eq!(status.code(), tonic::Code::FailedPrecondition, "{status}");
    assert!(
        status.message().contains("asset dataset"),
        "the count-limit error should name the asset dataset: {status}"
    );
}

/// Generate `len` bytes that LZ4 cannot shrink, so the on-disk size of a chunk holding them
/// tracks `len`.
fn incompressible_bytes(len: usize) -> Vec<u8> {
    // Knuth's MMIX linear congruential generator.
    let mut blob = vec![0u8; len];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for byte in &mut blob {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *byte = (state >> 56) as u8;
    }
    blob
}

/// A single static entity holding `payload_bytes` of incompressible data.
fn static_blob_components(payload_bytes: usize) -> BTreeMap<EntityPath, Box<dyn AsComponents>> {
    let blob = incompressible_bytes(payload_bytes);
    BTreeMap::from([(
        EntityPath::from("static/blob"),
        Box::new(AnyValues::default().with_component_from_data(
            "blob",
            Arc::new(BinaryArray::from_iter_values([blob.as_slice()])),
        )) as Box<dyn AsComponents>,
    )])
}

/// Create an asset recording holding a single static blob of `payload_bytes` of incompressible
/// data, so its compressed on-disk size stays close to `payload_bytes`.
fn create_static_recording_of_size(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    payload_bytes: usize,
) -> TempPath {
    create_recording_with_static_components(
        tuid_prefix,
        segment_id,
        static_blob_components(payload_bytes),
    )
    .unwrap()
}

/// Like [`create_static_recording_of_size`], but writes a blueprint store, since blueprint
/// datasets only load blueprint stores from a registered file.
fn create_blueprint_of_size(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    payload_bytes: usize,
) -> TempPath {
    create_blueprint_with_static_components(
        tuid_prefix,
        segment_id,
        static_blob_components(payload_bytes),
    )
    .unwrap()
}

/// An asset dataset accepts a segment just under the per-segment byte limit and rejects one just
/// over it. Both servers measure the same compressed on-disk size and reject the oversized one in
/// the registration task.
pub async fn asset_dataset_enforces_segment_size_limit(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;

    let limit = DatasetKind::Asset
        .limits()
        .max_segment_size_bytes
        .expect("asset datasets have a per-segment size limit");

    // LZ4 stores incompressible data as literal runs, expanding it by 1/255 plus a little framing.
    // Both servers measure that same expanded size, so it is what the margin must absorb: roughly
    // `limit / 256`, about 1.2 MiB at the current limit. 2 MiB keeps some headroom on top.
    let margin = 2 * 1024 * 1024;

    // Hold the temp recordings alive for the duration of the test.
    let mut recordings = Vec::new();

    let under = create_static_recording_of_size(
        100,
        "under_limit_asset",
        usize::try_from(limit - margin).unwrap(),
    );
    service
        .register_with_dataset_name_blocking(&asset_dataset_name, vec![rrd_data_source(&under)])
        .await;
    recordings.push(under);

    let over = create_static_recording_of_size(
        200,
        "over_limit_asset",
        usize::try_from(limit + margin).unwrap(),
    );
    let request = tonic::Request::new(RegisterWithDatasetRequest {
        data_sources: vec![rrd_data_source(&over)],
        on_duplicate: IfDuplicateBehavior::Error as i32,
    })
    .with_entry_name(entry_name(&asset_dataset_name));

    let task_results = register_and_wait(&service, request).await;
    // The message must name the asset dataset, not blueprints or plain segments.
    assert_task_failed(&task_results, "-byte limit for asset datasets");
    recordings.push(over);
}

/// A blueprint dataset accepts a blueprint under the per-segment byte limit and rejects one over
/// it, and the rejection names blueprint datasets rather than assets or plain segments.
pub async fn blueprint_dataset_enforces_segment_size_limit(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_blueprint";
    let dataset = service.create_dataset_entry_with_name(dataset_name).await;

    let blueprint_dataset_id = dataset
        .dataset_details
        .blueprint_dataset
        .expect("recording datasets should get an implicit blueprint dataset");
    let blueprint_dataset_name = dataset_entry_name(&service, blueprint_dataset_id).await;

    let limit = DatasetKind::Blueprint
        .limits()
        .max_segment_size_bytes
        .expect("blueprint datasets have a per-segment size limit");

    // See `asset_dataset_enforces_segment_size_limit` for why this margin is needed. The limit is
    // smaller here, so the LZ4 expansion it must absorb is only ~100 KiB.
    let margin = 1024 * 1024;

    // Hold the temp blueprints alive for the duration of the test.
    let mut blueprints = Vec::new();

    let under = create_blueprint_of_size(
        100,
        "under_limit_blueprint",
        usize::try_from(limit - margin).unwrap(),
    );
    service
        .register_with_dataset_name_blocking(&blueprint_dataset_name, vec![rrd_data_source(&under)])
        .await;
    blueprints.push(under);

    let over = create_blueprint_of_size(
        200,
        "over_limit_blueprint",
        usize::try_from(limit + margin).unwrap(),
    );
    let request = tonic::Request::new(RegisterWithDatasetRequest {
        data_sources: vec![rrd_data_source(&over)],
        on_duplicate: IfDuplicateBehavior::Error as i32,
    })
    .with_entry_name(entry_name(&blueprint_dataset_name));

    let task_results = register_and_wait(&service, request).await;
    assert_task_failed(&task_results, "-byte limit for blueprint datasets");
    blueprints.push(over);
}
