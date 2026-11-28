//! Registering RRD files that hold several stores.

use std::collections::BTreeSet;

use re_log_types::{ApplicationId, EntityPath, StoreId};
use re_protos::cloud::v1alpha1::RegisterWithDatasetRequest;
use re_protos::cloud::v1alpha1::ext::DataSource;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_sdk::external::re_log_encoding;
use re_types_core::SegmentId;

use super::common::{RerunCloudServiceExt as _, entry_name};
use super::segment_id_override::rrd_manifest;

/// The entity paths of the chunks of `segment_id`.
///
/// The stores of [`crate::create_recording_with_shared_recording_id_in`] each log to their own
/// entity, so this tells which of them a segment holds.
async fn segment_entity_paths(
    service: &impl RerunCloudService,
    dataset_name: &str,
    segment_id: &str,
) -> anyhow::Result<BTreeSet<EntityPath>> {
    let manifest = rrd_manifest(service, dataset_name, &SegmentId::from(segment_id)).await;
    Ok(manifest.col_chunk_entity_path_iter()?.collect())
}

/// A data source that names a store registers that store, whichever its place in the file. Two
/// stores that share a recording id can both be registered, under distinct segment ids.
pub async fn register_store_by_id(service: impl RerunCloudService) -> anyhow::Result<()> {
    let dataset_name = "dataset_register_store_by_id";
    let dir = tempfile::tempdir()?;
    let (path, store_ids) = crate::create_recording_with_shared_recording_id_in(
        730,
        "shared_recording",
        &["app_one", "app_two"],
        dir.path(),
    )?;
    let [app_one, app_two] =
        <[StoreId; 2]>::try_from(store_ids).unwrap_or_else(|_| panic!("two store ids"));
    let url = url::Url::from_file_path(&path)
        .map_err(|()| anyhow::anyhow!("invalid file path: {}", path.display()))?;

    service.create_dataset_entry_with_name(dataset_name).await;

    let second = DataSource::new_rrd_url(url.clone(), None).with_store_id(app_two);
    let first = DataSource::new_rrd_url(url, None)
        .with_store_id(app_one)
        .with_segment_id_override("other_segment".into());
    service
        .register_with_dataset_name_blocking(dataset_name, vec![second.into(), first.into()])
        .await;

    assert_eq!(
        segment_entity_paths(&service, dataset_name, "shared_recording").await?,
        BTreeSet::from([EntityPath::from("/app_two")]),
        "the segment named after the recording id must hold the store that was named"
    );
    assert_eq!(
        segment_entity_paths(&service, dataset_name, "other_segment").await?,
        BTreeSet::from([EntityPath::from("/app_one")]),
        "the overridden segment must hold the store that was named"
    );

    Ok(())
}

/// A data source cannot name a store that its file does not hold, nor one of another kind than
/// the dataset's.
pub async fn register_store_by_id_rejects_unusable_store(
    service: impl RerunCloudService,
) -> anyhow::Result<()> {
    let dataset_name = "dataset_register_store_by_id_rejects";
    let file_url = |path: &std::path::Path| {
        url::Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("invalid file path: {}", path.display()))
    };

    let dir = tempfile::tempdir()?;
    let (recordings_path, _) = crate::create_recording_with_shared_recording_id_in(
        731,
        "shared_recording",
        &["app_one", "app_two"],
        dir.path(),
    )?;
    let absent = StoreId::recording(ApplicationId::try_new("app_three")?, "shared_recording");

    // The blueprint store is one the file holds, so that only its kind can get it refused.
    let blueprint_path = crate::create_simple_blueprint(732, "my_blueprint")?;
    let blueprint =
        re_log_encoding::enumerate_rrd_stores(&std::fs::File::open(blueprint_path.as_path())?)
            .await?
            .into_iter()
            .find(StoreId::is_blueprint)
            .ok_or_else(|| anyhow::anyhow!("the blueprint file must hold a blueprint store"))?;

    service.create_dataset_entry_with_name(dataset_name).await;

    for (url, store_id, case) in [
        (file_url(&recordings_path)?, absent, "absent store"),
        (
            file_url(blueprint_path.as_path())?,
            blueprint,
            "blueprint store",
        ),
    ] {
        let data_source = DataSource::new_rrd_url(url, None).with_store_id(store_id);
        let status = service
            .register_with_dataset(
                tonic::Request::new(RegisterWithDatasetRequest {
                    data_sources: vec![data_source.into()],
                    on_duplicate: re_protos::common::v1alpha1::IfDuplicateBehavior::Error as i32,
                })
                .with_entry_name(entry_name(dataset_name)),
            )
            .await
            .expect_err(case);
        assert_eq!(
            status.code(),
            tonic::Code::InvalidArgument,
            "{case}: {status}"
        );
    }

    Ok(())
}
