use super::*;

use futures::TryStreamExt as _;
use re_protos::cloud::v1alpha1::GetAssetsForSegmentRequest;
use re_protos::headers::RerunHeadersInjectorExt as _;

fn build_handler() -> RerunCloudHandler {
    let builder = RerunCloudHandlerBuilder::new(re_build_info::cargo_build_info!());
    cfg_select! {
        target_arch = "wasm32" => builder.build(),
        _ => builder
            .build()
            .expect("failed to build handler"),
    }
}

/// The storage directory is created by `build`, is unique to this process, and is removed
/// together with the handler.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn storage_dir_lives_as_long_as_the_handler() {
    let handler = build_handler();
    let storage_dir = handler.storage_dir().to_owned();
    assert!(storage_dir.is_dir());

    let dir_name = storage_dir.file_name().unwrap().to_string_lossy();
    let expected_prefix = format!("rerun-data-{}-", std::process::id());
    assert!(
        dir_name.starts_with(&expected_prefix),
        "{dir_name:?} should start with {expected_prefix:?}"
    );

    drop(handler);
    assert!(!storage_dir.exists());
}

/// Datasets created before asset datasets were introduced don't have one. Querying assets on
/// such a dataset returns no assets, and updating its entry creates the missing asset dataset.
#[tokio::test]
async fn legacy_dataset_without_asset_dataset() {
    let handler = build_handler();

    let dataset_id = EntryId::new();
    handler
        .store
        .write()
        .await
        .create_dataset_impl(
            EntryName::new("legacy_dataset").unwrap(),
            dataset_id,
            DatasetKind::Recording,
            None,
        )
        .unwrap();

    let responses: Vec<_> = handler
        .get_assets_for_segment(
            tonic::Request::new(GetAssetsForSegmentRequest::default()).with_entry_id(dataset_id),
        )
        .await
        .expect("querying assets should succeed without an asset dataset")
        .into_inner()
        .try_collect()
        .await
        .unwrap();
    assert!(
        responses.is_empty(),
        "a dataset without an asset dataset should have no assets"
    );

    let updated: ext::DatasetEntry = handler
        .update_dataset_entry(tonic::Request::new(
            UpdateDatasetEntryRequest {
                id: dataset_id,
                dataset_details: Default::default(),
            }
            .into(),
        ))
        .await
        .expect("updating the entry should succeed")
        .into_inner()
        .dataset
        .unwrap()
        .try_into()
        .unwrap();

    let asset_dataset_id = updated
        .dataset_details
        .asset_dataset
        .expect("updating the entry should create the missing asset dataset");
    let store = handler.store.read().await;
    assert_eq!(
        store.dataset(asset_dataset_id).unwrap().dataset_kind(),
        DatasetKind::Asset,
    );
}
