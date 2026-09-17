#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
use futures::TryStreamExt as _;
use re_log_types::EntityPath;
use re_protos::cloud::v1alpha1::ext::{
    AssetMode, DataSource as DataSourceExt, DatasetDetails, QueryTasksDataframe,
    ScanDatasetManifestDataframe, asset_properties, read_asset_mode, read_asset_segments,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    DataSource, DeleteEntryRequest, EntryKind, GetAssetsForSegmentRequest,
    GetSegmentPropertiesRequest, ReadDatasetEntryRequest, RegisterWithDatasetRequest,
    ScanDatasetManifestRequest, SetSegmentPropertiesRequest,
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
            tonic::Request::new(GetAssetsForSegmentRequest::default())
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
                tonic::Request::new(GetAssetsForSegmentRequest::default())
                    .with_entry_id(non_recording),
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

/// Ask the server which asset segments apply to `segment_id`, returning them sorted by id.
async fn assets_for_segment(
    service: &impl RerunCloudService,
    dataset_name: &str,
    segment_id: &str,
) -> Vec<SegmentId> {
    let responses: Vec<_> = service
        .get_assets_for_segment(
            tonic::Request::new(GetAssetsForSegmentRequest {
                segment_id: Some(SegmentId::from(segment_id)),
            })
            .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .expect("get_assets_for_segment should succeed")
        .into_inner()
        .try_collect()
        .await
        .expect("get_assets_for_segment stream should succeed");

    let asset_dataset_name = asset_dataset_name(service, dataset_name).await;
    let mut ids = Vec::new();
    for id in responses
        .into_iter()
        .flat_map(|assets| assets.asset_segment_ids)
    {
        let asset_id = id.id.as_deref().expect("asset segment id should be set");
        let Some((mode, segments, _)) =
            asset_coverage(service, &asset_dataset_name, asset_id).await
        else {
            ids.push(id);
            continue;
        };
        if re_protos::cloud::v1alpha1::ext::asset_applies_to_segment(
            mode,
            Some(segment_id),
            &segments.into_iter().collect(),
        ) {
            ids.push(id);
        }
    }
    ids.sort_by(|a, b| a.id.cmp(&b.id));
    ids
}

/// Write an asset's coverage, returning the revision the write produced.
async fn set_asset_coverage(
    service: &impl RerunCloudService,
    asset_dataset_name: &str,
    asset_segment_id: &str,
    mode: AssetMode,
    segments: &[&str],
    expected_revision: Option<u64>,
) -> tonic::Result<u64> {
    service
        .set_segment_properties(
            tonic::Request::new(SetSegmentPropertiesRequest {
                segment_id: Some(SegmentId::from(asset_segment_id)),
                properties: Some((&asset_properties(mode, segments.iter().copied())).into()),
                expected_revision,
            })
            .with_entry_name(entry_name(asset_dataset_name)),
        )
        .await
        .map(|response| response.into_inner().revision)
}

/// Read back an asset's stored coverage, or `None` if it has no properties.
async fn asset_coverage(
    service: &impl RerunCloudService,
    asset_dataset_name: &str,
    asset_segment_id: &str,
) -> Option<(AssetMode, Vec<String>, u64)> {
    let responses: Vec<_> = service
        .get_segment_properties(
            tonic::Request::new(GetSegmentPropertiesRequest {
                segment_ids: vec![SegmentId::from(asset_segment_id)],
            })
            .with_entry_name(entry_name(asset_dataset_name)),
        )
        .await
        .expect("get_segment_properties should succeed")
        .into_inner()
        .try_collect()
        .await
        .expect("get_segment_properties stream should succeed");

    let segment = responses
        .into_iter()
        .flat_map(|response| response.segments)
        .find(|segment| segment.segment_id == Some(SegmentId::from(asset_segment_id)))?;

    let batch: RecordBatch = segment
        .properties
        .expect("stored properties should carry a batch")
        .try_into()
        .expect("stored properties should decode");
    let mut segments: Vec<String> = read_asset_segments(&batch, 0).into_iter().collect();
    segments.sort();
    Some((read_asset_mode(&batch, 0), segments, segment.revision))
}

/// A static-only layer holding a small blob, which is the only shape an asset dataset accepts.
fn asset_layer(segment_id: &'static str) -> LayerDefinition {
    LayerDefinition::static_components(
        segment_id,
        [(
            EntityPath::from("mesh"),
            Box::new(
                AnyValues::default().with_component_from_data(
                    "blob",
                    Arc::new(BinaryArray::from(vec![&b"asset"[..]])),
                ),
            ) as Box<dyn AsComponents>,
        )],
    )
}

/// Register one asset into `dataset_name`'s asset dataset, returning the asset dataset's name.
async fn register_one_asset(
    service: &impl RerunCloudService,
    dataset_name: &str,
    asset_segment_id: &'static str,
) -> String {
    let asset_dataset_name = asset_dataset_name(service, dataset_name).await;
    let assets = DataSourcesDefinition::new_with_tuid_prefix(100, [asset_layer(asset_segment_id)]);
    service
        .register_with_dataset_name_blocking(&asset_dataset_name, assets.to_data_sources())
        .await;
    asset_dataset_name
}

/// An `OptOut` asset applies to every segment except the ones it lists, while an `OptIn` asset
/// applies only to the ones it lists. Segments carry no asset properties of their own, so resolving
/// a segment's assets is a filter over the asset dataset alone.
pub async fn get_assets_for_segment_filters_by_properties(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    // Two plain segments in the main dataset, holding no asset properties.
    let main_segments = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("seg_default", &["my/entity"]),
            LayerDefinition::simple("seg_custom", &["my/entity"]),
        ],
    );
    service
        .register_with_dataset_name_blocking(dataset_name, main_segments.to_data_sources())
        .await;

    // Two assets, with their coverage written as properties.
    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;
    let assets = DataSourcesDefinition::new_with_tuid_prefix(
        100,
        [asset_layer("shared_asset"), asset_layer("special_asset")],
    );
    service
        .register_with_dataset_name_blocking(&asset_dataset_name, assets.to_data_sources())
        .await;

    // Both list `seg_custom`: `shared_asset` opts it out, `special_asset` opts it in.
    set_asset_coverage(
        &service,
        &asset_dataset_name,
        "shared_asset",
        AssetMode::OptOut,
        &["seg_custom"],
        None,
    )
    .await
    .expect("writing asset coverage should succeed");
    set_asset_coverage(
        &service,
        &asset_dataset_name,
        "special_asset",
        AssetMode::OptIn,
        &["seg_custom"],
        None,
    )
    .await
    .expect("writing asset coverage should succeed");

    assert_eq!(
        assets_for_segment(&service, dataset_name, "seg_default").await,
        vec![SegmentId::from("shared_asset")],
        "the default segment should get only the opt-out asset"
    );

    assert_eq!(
        assets_for_segment(&service, dataset_name, "seg_custom").await,
        vec![SegmentId::from("special_asset")],
        "the custom segment should get only the asset that opts it in"
    );
}

/// An asset registered without properties applies to every segment, since a missing `asset`
/// property resolves to the opt-out default.
pub async fn get_assets_for_segment_treats_missing_properties_as_opt_out(
    service: impl RerunCloudService,
) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let main_segments = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("seg_any", &["my/entity"])],
    );
    service
        .register_with_dataset_name_blocking(dataset_name, main_segments.to_data_sources())
        .await;

    let asset_dataset_name = register_one_asset(&service, dataset_name, "bare_asset").await;
    assert!(
        asset_coverage(&service, &asset_dataset_name, "bare_asset")
            .await
            .is_none(),
        "a freshly registered asset should hold no properties"
    );

    assert_eq!(
        assets_for_segment(&service, dataset_name, "seg_any").await,
        vec![SegmentId::from("bare_asset")],
        "an asset without properties should apply to every segment"
    );
}

/// Changing which segments an asset applies to is a property write, so it takes effect without
/// registering anything and without adding a segment or a layer to the asset dataset.
pub async fn set_segment_properties_changes_asset_coverage_without_registering(
    service: impl RerunCloudService,
) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let main_segments = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("seg_a", &["my/entity"]),
            LayerDefinition::simple("seg_b", &["my/entity"]),
        ],
    );
    service
        .register_with_dataset_name_blocking(dataset_name, main_segments.to_data_sources())
        .await;

    let asset_dataset_name = register_one_asset(&service, dataset_name, "the_asset").await;
    let layers_before = asset_dataset_layers(&service, &asset_dataset_name).await;

    let revision = set_asset_coverage(
        &service,
        &asset_dataset_name,
        "the_asset",
        AssetMode::OptIn,
        &["seg_a"],
        None,
    )
    .await
    .expect("writing asset coverage should succeed");
    assert_eq!(revision, 1, "the first write should be revision 1");

    assert_eq!(
        assets_for_segment(&service, dataset_name, "seg_a").await,
        vec![SegmentId::from("the_asset")]
    );
    assert_eq!(
        assets_for_segment(&service, dataset_name, "seg_b").await,
        vec![],
        "an opt-in asset should not apply to a segment it doesn't list"
    );

    // Widen the coverage, then narrow it back. Each edit is one write.
    let revision = set_asset_coverage(
        &service,
        &asset_dataset_name,
        "the_asset",
        AssetMode::OptIn,
        &["seg_a", "seg_b"],
        Some(revision),
    )
    .await
    .expect("widening asset coverage should succeed");
    assert_eq!(revision, 2, "each write should bump the revision");
    assert_eq!(
        assets_for_segment(&service, dataset_name, "seg_b").await,
        vec![SegmentId::from("the_asset")]
    );

    set_asset_coverage(
        &service,
        &asset_dataset_name,
        "the_asset",
        AssetMode::OptIn,
        &["seg_b"],
        Some(revision),
    )
    .await
    .expect("narrowing asset coverage should succeed");
    assert_eq!(
        assets_for_segment(&service, dataset_name, "seg_a").await,
        vec![],
        "dropping a segment from the list should stop the asset applying to it"
    );

    assert_eq!(
        asset_dataset_layers(&service, &asset_dataset_name).await,
        layers_before,
        "editing coverage should not add segments or layers to the asset dataset"
    );
}

/// A write gated on a stale revision is refused and changes nothing, so a caller that lost a race
/// can re-read and recompute.
pub async fn set_segment_properties_refuses_a_stale_revision(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset_name = register_one_asset(&service, dataset_name, "the_asset").await;

    let first = set_asset_coverage(
        &service,
        &asset_dataset_name,
        "the_asset",
        AssetMode::OptIn,
        &["seg_a"],
        Some(0),
    )
    .await
    .expect("expecting revision 0 should create the properties");
    assert_eq!(first, 1);

    set_asset_coverage(
        &service,
        &asset_dataset_name,
        "the_asset",
        AssetMode::OptIn,
        &["winner"],
        Some(first),
    )
    .await
    .expect("expecting the current revision should succeed");

    let status = set_asset_coverage(
        &service,
        &asset_dataset_name,
        "the_asset",
        AssetMode::OptIn,
        &["loser"],
        Some(first),
    )
    .await
    .expect_err("expecting a stale revision should fail");
    assert_eq!(
        status.code(),
        tonic::Code::FailedPrecondition,
        "unexpected status: {status}"
    );

    let (mode, segments, revision) = asset_coverage(&service, &asset_dataset_name, "the_asset")
        .await
        .expect("the properties should still be there");
    assert_eq!(mode, AssetMode::OptIn);
    assert_eq!(
        segments,
        vec!["winner".to_owned()],
        "the refused write must not have landed"
    );
    assert_eq!(revision, 2);
}

/// Unregistering a segment preserves its properties and revision.
/// Registering the same segment ID again reuses those properties.
pub async fn unregistering_a_segment_preserves_its_properties(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let main_segments = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("seg_a", &["my/entity"])],
    );
    service
        .register_with_dataset_name_blocking(dataset_name, main_segments.to_data_sources())
        .await;

    let asset_dataset_name = register_one_asset(&service, dataset_name, "recycled").await;
    set_asset_coverage(
        &service,
        &asset_dataset_name,
        "recycled",
        AssetMode::OptIn,
        &["seg_a"],
        None,
    )
    .await
    .expect("writing asset coverage should succeed");
    assert!(
        asset_coverage(&service, &asset_dataset_name, "recycled")
            .await
            .is_some(),
        "the asset should hold properties before it is unregistered"
    );

    let coverage = asset_coverage(&service, &asset_dataset_name, "recycled").await;
    service
        .unregister_from_dataset_name_blocking(&asset_dataset_name, &["recycled"], &[])
        .await
        .expect("unregister should succeed");

    assert!(
        asset_dataset_layers(&service, &asset_dataset_name)
            .await
            .is_empty()
    );
    assert_eq!(
        asset_coverage(&service, &asset_dataset_name, "recycled").await,
        coverage,
    );

    register_one_asset(&service, dataset_name, "recycled").await;
    assert_eq!(
        asset_coverage(&service, &asset_dataset_name, "recycled").await,
        coverage,
    );
}

/// Properties are one row per segment, so a batch carrying several rows is refused as a caller
/// mistake.
pub async fn set_segment_properties_rejects_a_multi_row_batch(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset_name = register_one_asset(&service, dataset_name, "the_asset").await;

    let two_rows = RecordBatch::try_from_iter([(
        "property:asset:mode",
        Arc::new(StringArray::from(vec!["OptIn", "OptOut"])) as ArrayRef,
    )])
    .expect("a one-column batch is valid");

    let status = service
        .set_segment_properties(
            tonic::Request::new(SetSegmentPropertiesRequest {
                segment_id: Some(SegmentId::from("the_asset")),
                properties: Some((&two_rows).into()),
                expected_revision: None,
            })
            .with_entry_name(entry_name(&asset_dataset_name)),
        )
        .await
        .expect_err("a multi-row batch must be rejected");
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status}"
    );

    assert!(
        asset_coverage(&service, &asset_dataset_name, "the_asset")
            .await
            .is_none(),
        "the refused write must not have stored anything"
    );
}

/// A request naming several segments reads the properties of each of them. A named segment
/// carrying none is absent from the response.
pub async fn get_segment_properties_reads_several_segments(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;
    let assets = DataSourcesDefinition::new_with_tuid_prefix(
        100,
        [
            asset_layer("with_properties_a"),
            asset_layer("with_properties_b"),
            asset_layer("without_properties"),
        ],
    );
    service
        .register_with_dataset_name_blocking(&asset_dataset_name, assets.to_data_sources())
        .await;

    for asset_segment_id in ["with_properties_a", "with_properties_b"] {
        set_asset_coverage(
            &service,
            &asset_dataset_name,
            asset_segment_id,
            AssetMode::OptIn,
            &["seg_a"],
            None,
        )
        .await
        .expect("writing asset coverage should succeed");
    }

    let responses: Vec<_> = service
        .get_segment_properties(
            tonic::Request::new(GetSegmentPropertiesRequest {
                segment_ids: [
                    "with_properties_a",
                    "with_properties_b",
                    "without_properties",
                ]
                .into_iter()
                .map(SegmentId::from)
                .collect(),
            })
            .with_entry_name(entry_name(&asset_dataset_name)),
        )
        .await
        .expect("get_segment_properties should succeed")
        .into_inner()
        .try_collect::<Vec<_>>()
        .await
        .expect("get_segment_properties stream should succeed");

    let mut segment_ids: Vec<String> = responses
        .into_iter()
        .flat_map(|response| response.segments)
        .filter_map(|segment| segment.segment_id?.id)
        .collect();
    segment_ids.sort();
    assert_eq!(
        segment_ids,
        vec![
            "with_properties_a".to_owned(),
            "with_properties_b".to_owned()
        ],
        "the response should carry the named segments that have properties, and only those"
    );
}

/// A read that names no segment is refused.
pub async fn get_segment_properties_rejects_an_empty_request(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;
    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;

    let status = service
        .get_segment_properties(
            tonic::Request::new(GetSegmentPropertiesRequest {
                segment_ids: vec![],
            })
            .with_entry_name(entry_name(&asset_dataset_name)),
        )
        .await
        .err()
        .expect("get_segment_properties should fail");
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// Dropping layers preserves properties for both removed segments and segments with surviving layers.
pub async fn dropping_the_last_layer_of_a_segment_preserves_its_properties(
    service: impl RerunCloudService,
) {
    let dataset_name = "dataset_with_asset";
    service.create_dataset_entry_with_name(dataset_name).await;

    // `emptied` holds one layer and loses it, `kept` holds two and loses one.
    let asset_dataset_name = asset_dataset_name(&service, dataset_name).await;
    let assets = DataSourcesDefinition::new_with_tuid_prefix(
        100,
        [
            asset_layer("emptied").layer_name("only_layer"),
            asset_layer("kept").layer_name("dropped_layer"),
            asset_layer("kept").layer_name("surviving_layer"),
        ],
    );
    service
        .register_with_dataset_name_blocking(&asset_dataset_name, assets.to_data_sources())
        .await;

    for asset_segment_id in ["emptied", "kept"] {
        set_asset_coverage(
            &service,
            &asset_dataset_name,
            asset_segment_id,
            AssetMode::OptIn,
            &["seg_a"],
            None,
        )
        .await
        .expect("writing asset coverage should succeed");
    }

    // Naming no segment drops the layers from every segment of the dataset.
    service
        .unregister_from_dataset_name_blocking(
            &asset_dataset_name,
            &[],
            &["only_layer", "dropped_layer"],
        )
        .await
        .expect("unregister should succeed");

    assert_eq!(
        asset_dataset_layers(&service, &asset_dataset_name).await,
        vec![("kept".to_owned(), "surviving_layer".to_owned())],
    );
    assert_eq!(
        asset_coverage(&service, &asset_dataset_name, "emptied").await,
        Some((AssetMode::OptIn, vec!["seg_a".to_owned()], 1)),
    );
    assert!(
        asset_coverage(&service, &asset_dataset_name, "kept")
            .await
            .is_some(),
        "a segment that still holds a layer should have kept its properties"
    );
}

/// The (segment, layer) pairs of an asset dataset, sorted, so tests can assert that editing
/// properties leaves the dataset's structure alone.
async fn asset_dataset_layers(
    service: &impl RerunCloudService,
    asset_dataset_name: &str,
) -> Vec<(String, String)> {
    let responses: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest {
                columns: vec![
                    ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID
                        .name
                        .to_owned(),
                    ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME
                        .name
                        .to_owned(),
                ],
                ..Default::default()
            })
            .with_entry_name(entry_name(asset_dataset_name)),
        )
        .await
        .expect("scan_dataset_manifest should succeed")
        .into_inner()
        .try_collect()
        .await
        .expect("scan_dataset_manifest stream should succeed");

    let mut out = Vec::new();
    for response in responses {
        let Some(data) = response.data else {
            continue;
        };
        let batch: RecordBatch = data.try_into().expect("manifest should decode");
        let segment_ids = ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID
            .extract(&batch)
            .expect("valid segment id column");
        let layer_names = ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME
            .extract(&batch)
            .expect("valid layer name column");
        for (segment_id, layer_name) in std::iter::zip(&segment_ids, &layer_names) {
            out.push((segment_id.to_owned(), layer_name.to_owned()));
        }
    }
    out.sort();
    out
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
