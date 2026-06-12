use arrow::array::FixedSizeBinaryArray;
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_protos::cloud::v1alpha1::ext::{self, DataSource, QueryDatasetRequest};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{RegisterWithDatasetRequest, ScanDatasetManifestRequest};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_types_core::{ChunkId, LayerName};
use url::Url;

use super::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};
use crate::{TempPath, create_simple_recording_in};

type Result<T = ()> = anyhow::Result<T>;

// --- helpers ---

fn create_rrd(tuid_prefix: u64, segment_id: &str, entities: &[&str]) -> Result<TempPath> {
    let tmp_dir = tempfile::tempdir()?;
    let path = create_simple_recording_in(
        tuid_prefix,
        segment_id,
        entities,
        0,
        re_log_types::TimeType::Sequence,
        tmp_dir.path(),
    )?;
    Ok(TempPath::new(tmp_dir, path))
}

fn file_url(path: &TempPath) -> Url {
    Url::from_file_path(path.as_path()).expect("absolute path")
}

fn asset_layer_request(
    dataset_name: &str,
    layer_name: &str,
    url: Url,
    on_duplicate: IfDuplicateBehavior,
) -> tonic::Request<RegisterWithDatasetRequest> {
    tonic::Request::new(RegisterWithDatasetRequest {
        data_sources: vec![DataSource::new_rrd_asset_layer(layer_name, url).into()],
        on_duplicate: re_protos::common::v1alpha1::IfDuplicateBehavior::from(on_duplicate) as i32,
    })
    .with_entry_name(entry_name(dataset_name))
}

fn segment_layer_request(
    dataset_name: &str,
    layer_name: &str,
    url: &Url,
    on_duplicate: IfDuplicateBehavior,
) -> Result<tonic::Request<RegisterWithDatasetRequest>> {
    Ok(tonic::Request::new(RegisterWithDatasetRequest {
        data_sources: vec![DataSource::new_rrd_layer(layer_name, url.as_str())?.into()],
        on_duplicate: re_protos::common::v1alpha1::IfDuplicateBehavior::from(on_duplicate) as i32,
    })
    .with_entry_name(entry_name(dataset_name)))
}

async fn register_asset_layer(
    service: &impl RerunCloudService,
    dataset_name: &str,
    layer_name: &str,
    url: Url,
    on_duplicate: IfDuplicateBehavior,
) -> Result {
    service
        .register_with_dataset(asset_layer_request(
            dataset_name,
            layer_name,
            url,
            on_duplicate,
        ))
        .await?;
    Ok(())
}

async fn scan_manifest(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> Result<arrow::array::RecordBatch> {
    let responses: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest {
                columns: vec![], // all
            })
            .with_entry_name(entry_name(dataset_name)),
        )
        .await?
        .into_inner()
        .try_collect()
        .await?;

    let mut batches: Vec<arrow::array::RecordBatch> = Vec::new();
    for resp in responses {
        batches.push(resp.data.expect("response is missing data").try_into()?);
    }

    let schema = batches
        .first()
        .expect("empty manifest response")
        .schema_ref()
        .clone();
    Ok(arrow::compute::concat_batches(&schema, &batches)?)
}

fn manifest_layer_names(batch: &arrow::array::RecordBatch) -> Vec<String> {
    ext::ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME
        .extract(batch)
        .expect("valid layer name column")
        .into_iter()
        .map(LayerName::into_string)
        .sorted()
        .collect()
}

/// The segment ids of all manifest rows with the given layer name, sorted.
fn segment_ids_of_layer(batch: &arrow::array::RecordBatch, layer_name: &str) -> Vec<String> {
    let layer_names = ext::ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME
        .extract(batch)
        .expect("valid layer name column");
    let segment_ids = ext::ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID
        .extract(batch)
        .expect("valid segment id column");
    std::iter::zip(layer_names, segment_ids)
        .filter(|(name, _)| name == layer_name)
        .map(|(_, segment_id)| String::from(segment_id))
        .sorted()
        .collect()
}

fn collect_chunk_ids(batches: &[arrow::array::RecordBatch]) -> Vec<ChunkId> {
    use re_protos::cloud::v1alpha1::QueryDatasetResponse;
    let mut ids = Vec::new();
    for batch in batches {
        let col = batch
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
            .expect("missing chunk id column");
        let arr = col
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .expect("expected a FixedSizeBinaryArray");
        ids.extend_from_slice(ChunkId::try_slice_from_arrow(arr).expect("invalid chunk ids"));
    }
    ids
}

async fn query_entity(
    service: &impl RerunCloudService,
    dataset_name: &str,
    segment_id: &str,
    entity_path: &str,
) -> Result<Vec<arrow::array::RecordBatch>> {
    let responses: Vec<_> = service
        .query_dataset(
            tonic::Request::new(
                QueryDatasetRequest {
                    segment_ids: vec![segment_id.to_owned().into()],
                    entity_paths: vec![entity_path.into()],
                    select_all_entity_paths: false,
                    ..Default::default()
                }
                .into(),
            )
            .with_entry_name(entry_name(dataset_name)),
        )
        .await?
        .into_inner()
        .try_collect()
        .await?;

    let mut batches = Vec::new();
    for resp in responses {
        if let Some(dfp) = resp.data {
            batches.push(dfp.try_into()?);
        }
    }
    Ok(batches)
}

// --- tests ---

/// An asset layer appears in the manifest once per segment.
///
/// TODO(RR-4807): consider this choice — a corollary is that an asset layer registered
/// to a dataset without segments is invisible in the manifest.
pub async fn register_asset_layer_appears_in_manifest(service: impl RerunCloudService) -> Result {
    let asset = create_rrd(1, "asset_recording_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    register_asset_layer(
        &service,
        dataset_name,
        "robot_urdf",
        asset_url,
        IfDuplicateBehavior::Error,
    )
    .await?;

    // No segments yet, so the asset layer is invisible in the manifest.
    assert_eq!(
        scan_manifest(&service, dataset_name).await?.num_rows(),
        0,
        "asset layer should be invisible in the manifest of a segment-less dataset"
    );

    // Register a segment; the asset layer should now appear for it.
    let segments_def = DataSourcesDefinition::new_with_tuid_prefix(
        100,
        [LayerDefinition::simple("seg1", &["my/entity"])],
    );
    service
        .register_with_dataset_name_blocking(dataset_name, segments_def.to_data_sources())
        .await;

    let manifest = scan_manifest(&service, dataset_name).await?;
    assert_eq!(
        manifest_layer_names(&manifest),
        ["base", "robot_urdf"],
        "asset layer should appear in the manifest alongside the segment layer"
    );
    assert_eq!(
        segment_ids_of_layer(&manifest, "robot_urdf"),
        ["seg1"],
        "the asset layer row should carry the segment's id"
    );
    Ok(())
}

/// An asset layer and regular segment layers coexist in the manifest.
pub async fn register_asset_layer_coexists_with_segment_layers(
    service: impl RerunCloudService,
) -> Result {
    let segments_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("seg1", &["my/entity"]),
            LayerDefinition::simple("seg2", &["my/entity"]),
        ],
    );

    // Asset uses a separate tuid prefix to avoid chunk-id collision with segments.
    let asset = create_rrd(100, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, segments_def.to_data_sources())
        .await;

    register_asset_layer(
        &service,
        dataset_name,
        "robot_urdf",
        asset_url,
        IfDuplicateBehavior::Error,
    )
    .await?;

    let manifest = scan_manifest(&service, dataset_name).await?;

    // The asset layer is listed once per segment, next to each segment's own layers.
    // TODO(RR-4807): consider this choice.
    assert_eq!(
        manifest_layer_names(&manifest),
        ["base", "base", "robot_urdf", "robot_urdf"],
        "expected one base row and one asset row per segment"
    );
    assert_eq!(
        segment_ids_of_layer(&manifest, "robot_urdf"),
        ["seg1", "seg2"],
        "the asset layer should appear in every segment"
    );
    Ok(())
}

/// Querying the dataset returns the asset layer chunks for every segment.
pub async fn query_dataset_asset_layer_included_in_all_segments(
    service: impl RerunCloudService,
) -> Result {
    let segments_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("seg1", &["my/entity"]),
            LayerDefinition::simple("seg2", &["my/entity"]),
        ],
    );

    // Asset uses a separate tuid prefix to avoid chunk-id collision with segments.
    let asset = create_rrd(100, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, segments_def.to_data_sources())
        .await;

    register_asset_layer(
        &service,
        dataset_name,
        "robot_urdf",
        asset_url,
        IfDuplicateBehavior::Error,
    )
    .await?;

    for segment_id in ["seg1", "seg2"] {
        let batches = query_entity(&service, dataset_name, segment_id, "/robot/urdf").await?;
        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert!(
            total_rows > 0,
            "segment '{segment_id}' should see asset layer chunks for /robot/urdf (got {total_rows} rows)"
        );
    }
    Ok(())
}

/// Re-registering an asset layer with `on_duplicate = Error` fails as expected.
pub async fn register_asset_layer_duplicate_error(service: impl RerunCloudService) -> Result {
    let asset = create_rrd(1, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let make_request = || {
        tonic::Request::new(RegisterWithDatasetRequest {
            data_sources: vec![
                DataSource::new_rrd_asset_layer("robot_urdf", asset_url.clone()).into(),
            ],
            on_duplicate: re_protos::common::v1alpha1::IfDuplicateBehavior::from(
                IfDuplicateBehavior::Error,
            ) as i32,
        })
        .with_entry_name(entry_name(dataset_name))
    };

    service.register_with_dataset(make_request()).await?;

    let err = service
        .register_with_dataset(make_request())
        .await
        .expect_err("second registration with Error policy should fail");
    assert_eq!(err.code(), tonic::Code::AlreadyExists);
    Ok(())
}

/// Re-registering an asset layer with `on_duplicate = Overwrite` succeeds,
/// also when the asset layer has already been propagated to existing segments.
pub async fn register_asset_layer_duplicate_overwrite(service: impl RerunCloudService) -> Result {
    let segments_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("seg1", &["my/entity"])],
    );

    // Asset uses a separate tuid prefix to avoid chunk-id collision with segments.
    let asset = create_rrd(100, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, segments_def.to_data_sources())
        .await;

    let make_request = || {
        tonic::Request::new(RegisterWithDatasetRequest {
            data_sources: vec![
                DataSource::new_rrd_asset_layer("robot_urdf", asset_url.clone()).into(),
            ],
            on_duplicate: re_protos::common::v1alpha1::IfDuplicateBehavior::from(
                IfDuplicateBehavior::Overwrite,
            ) as i32,
        })
        .with_entry_name(entry_name(dataset_name))
    };

    service.register_with_dataset(make_request()).await?;
    // The second registration overwrites the asset layer both in the dataset
    // and in the existing segment it was propagated to.
    service.register_with_dataset(make_request()).await?;

    let layer_names = manifest_layer_names(&scan_manifest(&service, dataset_name).await?);
    assert_eq!(
        layer_names
            .iter()
            .filter(|name| *name == "robot_urdf")
            .count(),
        1,
        "only one asset row should exist after overwrite (got {layer_names:?})"
    );
    Ok(())
}

/// Re-registering a layer under the same name but with a different class (segment ↔ asset)
/// works after unregistering the original entry first.
///
/// As a segment layer, `my_layer` only exists in its own segment.
/// As an asset layer, it appears in every segment (`anchor1`, `anchor2`, …).
pub async fn reregister_layer_change_class(service: impl RerunCloudService) -> Result {
    // Two anchor segments, so we can tell a segment layer (one segment)
    // from an asset layer (all segments) apart in the manifest.
    let anchors_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("anchor1", &["my/entity"]),
            LayerDefinition::simple("anchor2", &["my/entity"]),
        ],
    );

    // Separate tuid prefixes so chunk ids don't collide.
    let seg_rrd = create_rrd(100, "seg_id", &["my/entity"])?;
    let seg_url = file_url(&seg_rrd);

    let asset_rrd = create_rrd(200, "asset_id", &["my/entity"])?;
    let asset_url = file_url(&asset_rrd);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, anchors_def.to_data_sources())
        .await;

    // --- Segment → Asset ---

    service
        .register_with_dataset_name_blocking(
            dataset_name,
            vec![DataSource::new_rrd_layer("my_layer", seg_url.as_str())?.into()],
        )
        .await;

    let manifest = scan_manifest(&service, dataset_name).await?;
    assert_eq!(
        segment_ids_of_layer(&manifest, "my_layer"),
        ["seg_id"],
        "as a segment layer, 'my_layer' should only exist in its own segment"
    );

    service
        .unregister_from_dataset_name(dataset_name, &[], &["my_layer"])
        .await?;

    let manifest = scan_manifest(&service, dataset_name).await?;
    assert!(
        segment_ids_of_layer(&manifest, "my_layer").is_empty(),
        "'my_layer' should be gone after unregister"
    );

    register_asset_layer(
        &service,
        dataset_name,
        "my_layer",
        asset_url,
        IfDuplicateBehavior::Error,
    )
    .await?;

    let manifest = scan_manifest(&service, dataset_name).await?;
    assert_eq!(
        segment_ids_of_layer(&manifest, "my_layer"),
        ["anchor1", "anchor2"],
        "as an asset layer, 'my_layer' should appear in every segment"
    );

    // --- Asset → Segment ---

    service
        .unregister_from_dataset_name(dataset_name, &[], &["my_layer"])
        .await?;

    let manifest = scan_manifest(&service, dataset_name).await?;
    assert!(
        segment_ids_of_layer(&manifest, "my_layer").is_empty(),
        "'my_layer' should be gone after unregistering the asset layer"
    );

    service
        .register_with_dataset_name_blocking(
            dataset_name,
            vec![DataSource::new_rrd_layer("my_layer", seg_url.as_str())?.into()],
        )
        .await;

    let manifest = scan_manifest(&service, dataset_name).await?;
    assert_eq!(
        segment_ids_of_layer(&manifest, "my_layer"),
        ["seg_id"],
        "re-registered layer should be a segment layer again"
    );
    Ok(())
}

/// When querying multiple segments, the asset layer's chunk IDs appear identically in every
/// segment's result — the same chunk IDs are duplicated across segments.
pub async fn query_dataset_asset_chunk_ids_duplicated_across_segments(
    service: impl RerunCloudService,
) -> Result {
    let segments_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("seg1", &["my/entity"]),
            LayerDefinition::simple("seg2", &["my/entity"]),
        ],
    );

    // Asset uses a separate tuid prefix to avoid chunk-id collision with segments.
    let asset = create_rrd(100, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, segments_def.to_data_sources())
        .await;

    register_asset_layer(
        &service,
        dataset_name,
        "robot_urdf",
        asset_url,
        IfDuplicateBehavior::Error,
    )
    .await?;

    let ids_seg1 =
        collect_chunk_ids(&query_entity(&service, dataset_name, "seg1", "/robot/urdf").await?);
    let ids_seg2 =
        collect_chunk_ids(&query_entity(&service, dataset_name, "seg2", "/robot/urdf").await?);

    assert!(
        !ids_seg1.is_empty(),
        "seg1 should return asset layer chunks"
    );
    assert!(
        !ids_seg2.is_empty(),
        "seg2 should return asset layer chunks"
    );

    let common: Vec<_> = ids_seg1.iter().filter(|id| ids_seg2.contains(id)).collect();
    assert!(
        !common.is_empty(),
        "asset layer chunk IDs must be duplicated across segments (seg1={ids_seg1:?}, seg2={ids_seg2:?})"
    );
    Ok(())
}

/// Unregistering works for both asset layers and segment layers.
pub async fn unregister_asset_and_segment_layers(service: impl RerunCloudService) -> Result {
    let segments_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("seg1", &["my/entity"])],
    );

    let asset = create_rrd(100, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, segments_def.to_data_sources())
        .await;

    register_asset_layer(
        &service,
        dataset_name,
        "robot_urdf",
        asset_url,
        IfDuplicateBehavior::Error,
    )
    .await?;

    assert_eq!(
        scan_manifest(&service, dataset_name).await?.num_rows(),
        2,
        "should have 1 segment row + 1 asset row"
    );

    // Unregister the segment layer by layer name (no segment filter).
    service
        .unregister_from_dataset_name(dataset_name, &[], &["base"])
        .await?;

    assert_eq!(
        manifest_layer_names(&scan_manifest(&service, dataset_name).await?),
        ["robot_urdf"],
        "only the asset layer should remain after removing segment layer"
    );

    // Unregister the asset layer by layer name.
    service
        .unregister_from_dataset_name(dataset_name, &[], &["robot_urdf"])
        .await?;

    assert_eq!(
        scan_manifest(&service, dataset_name).await?.num_rows(),
        0,
        "manifest should be empty after removing both layers"
    );
    Ok(())
}

/// Registering an asset layer whose name collides with an existing *segment* layer
/// is rejected, regardless of `on_duplicate` — the layer classes differ.
pub async fn asset_layer_name_collision_with_segment_layer_errors(
    service: impl RerunCloudService,
) -> Result {
    // A segment with the segment layer "base":
    let segments_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("seg1", &["my/entity"])],
    );

    let asset = create_rrd(100, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    service
        .register_with_dataset_name_blocking(dataset_name, segments_def.to_data_sources())
        .await;

    for on_duplicate in [
        IfDuplicateBehavior::Error,
        IfDuplicateBehavior::Overwrite,
        IfDuplicateBehavior::Skip,
    ] {
        let err = service
            .register_with_dataset(asset_layer_request(
                dataset_name,
                "base",
                asset_url.clone(),
                on_duplicate,
            ))
            .await
            .expect_err("registering an asset layer named like a segment layer should fail");
        assert_eq!(err.code(), tonic::Code::AlreadyExists, "{on_duplicate:?}");
        assert!(
            err.message().contains("layer class"),
            "error should mention the layer class conflict, got: {}",
            err.message()
        );
    }

    // The segment layer should be untouched.
    assert_eq!(
        manifest_layer_names(&scan_manifest(&service, dataset_name).await?),
        ["base"],
        "the segment layer should be untouched by the failed registrations"
    );
    Ok(())
}

/// Registering a segment layer whose name collides with an existing *asset* layer
/// is rejected, regardless of `on_duplicate` — the layer classes differ.
pub async fn segment_layer_name_collision_with_asset_layer_errors(
    service: impl RerunCloudService,
) -> Result {
    let asset = create_rrd(1, "asset_id", &["robot/urdf"])?;
    let asset_url = file_url(&asset);

    let seg_rrd = create_rrd(100, "seg_id", &["my/entity"])?;
    let seg_url = file_url(&seg_rrd);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    register_asset_layer(
        &service,
        dataset_name,
        "shared_asset",
        asset_url,
        IfDuplicateBehavior::Error,
    )
    .await?;

    for on_duplicate in [
        IfDuplicateBehavior::Error,
        IfDuplicateBehavior::Overwrite,
        IfDuplicateBehavior::Skip,
    ] {
        let err = service
            .register_with_dataset(segment_layer_request(
                dataset_name,
                "shared_asset",
                &seg_url,
                on_duplicate,
            )?)
            .await
            .expect_err("registering a segment layer named like an asset layer should fail");
        assert_eq!(err.code(), tonic::Code::AlreadyExists, "{on_duplicate:?}");
        assert!(
            err.message().contains("layer class"),
            "error should mention the layer class conflict, got: {}",
            err.message()
        );
    }

    // A segment layer with a non-conflicting name still works,
    // and its new segment is seeded with the asset layer.
    service
        .register_with_dataset(segment_layer_request(
            dataset_name,
            "other_layer",
            &seg_url,
            IfDuplicateBehavior::Error,
        )?)
        .await?;

    assert_eq!(
        manifest_layer_names(&scan_manifest(&service, dataset_name).await?),
        ["other_layer", "shared_asset"],
        "the new segment should have its own layer plus the seeded asset layer"
    );
    Ok(())
}
