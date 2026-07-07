use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    CreateDatasetEntryRequest, DatasetDetails, DatasetEntry, EntryDetailsUpdate, TableDetails,
    TableEntry, UpdateDatasetEntryRequest, UpdateEntryRequest, UpdateEntryResponse,
    UpdateTableEntryRequest,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{DeleteEntryRequest, ReadTableEntryRequest};
use re_protos::common::v1alpha1::ext::SegmentId;

use super::common::{create_table_entry_with_name, entry_name};

pub async fn update_entry_tests(service: impl RerunCloudService) {
    //
    // Create a dataset
    //

    let dataset_name = "initial_name";

    let dataset_entry = create_dataset_entry(&service, dataset_name).await.unwrap();

    assert_eq!(dataset_entry.details.name, entry_name(dataset_name));

    let dataset_id = dataset_entry.details.id;

    //
    // No-op dataset update should succeed
    //

    let response = update_entry(
        &service,
        UpdateEntryRequest {
            id: dataset_id,
            entry_details_update: EntryDetailsUpdate { name: None },
        },
    )
    .await
    .unwrap();

    assert_eq!(response.entry_details.name, entry_name(dataset_name));

    //
    // Dataset rename should succeed
    //

    let new_dataset_name = "new_name";
    let response = update_entry(
        &service,
        UpdateEntryRequest {
            id: dataset_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(entry_name(new_dataset_name)),
            },
        },
    )
    .await
    .unwrap();

    assert_eq!(response.entry_details.name, entry_name(new_dataset_name));

    //
    // Create another dataset
    //

    let dataset2_name = "dataset_2";

    let dataset2_entry = create_dataset_entry(&service, dataset2_name).await.unwrap();

    let dataset2_id = dataset2_entry.details.id;

    //
    // Renaming to an existing name should fail.
    //

    let status = update_entry(
        &service,
        UpdateEntryRequest {
            id: dataset2_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(entry_name(new_dataset_name)),
            },
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        status.code(),
        tonic::Code::AlreadyExists,
        "unexpected status: {status:?}",
    );

    //
    // Create a table
    //

    let table_dir = tempfile::tempdir().expect("create temp dir");
    let table_name = "table_1";

    let table_entry = create_table_entry_with_name(&service, table_name, &table_dir).await;

    assert_eq!(table_entry.details.name, entry_name(table_name));
    let table_id = table_entry.details.id;

    //
    // Update table name
    //

    let new_table_name = "new_table_name";
    let response = update_entry(
        &service,
        UpdateEntryRequest {
            id: table_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(entry_name(new_table_name)),
            },
        },
    )
    .await
    .unwrap();

    assert_eq!(response.entry_details.name, entry_name(new_table_name));

    //
    // Updating table name to an existing dataset name should fail.
    //

    let status = update_entry(
        &service,
        UpdateEntryRequest {
            id: table_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(entry_name(dataset2_name)),
            },
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        status.code(),
        tonic::Code::AlreadyExists,
        "unexpected status: {status:?}",
    );

    //
    // Create another table.
    //

    let table2_name = "table_2";
    let table2_dir = tempfile::tempdir().expect("create temp dir");

    let table2_entry = create_table_entry_with_name(&service, table2_name, &table2_dir).await;
    let table2_id = table2_entry.details.id;

    //
    // Rename to an existing table name should fail.
    //

    let status = update_entry(
        &service,
        UpdateEntryRequest {
            id: table2_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(entry_name(new_table_name)),
            },
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        status.code(),
        tonic::Code::AlreadyExists,
        "unexpected status: {status:?}",
    );
}

pub async fn update_table_entry_blueprint_details(service: impl RerunCloudService) {
    let table_dir = tempfile::tempdir().expect("create temp dir");
    let table_entry =
        create_table_entry_with_name(&service, "table_with_blueprint", &table_dir).await;
    let table_id = table_entry.details.id;
    let blueprint_dataset = table_entry
        .table_details
        .blueprint_dataset
        .expect("tables should get an implicit blueprint dataset");
    let default_blueprint_segment = SegmentId::from("default_table_blueprint");

    let updated = update_table_entry(
        &service,
        UpdateTableEntryRequest {
            id: table_id,
            table_details: TableDetails {
                blueprint_dataset: Some(blueprint_dataset),
                default_blueprint_segment: Some(default_blueprint_segment.clone()),
            },
        },
    )
    .await
    .unwrap();

    assert_eq!(
        updated.table_details.blueprint_dataset,
        Some(blueprint_dataset)
    );
    assert_eq!(
        updated.table_details.default_blueprint_segment,
        Some(default_blueprint_segment.clone())
    );

    let read_back = read_table_entry(&service, table_id).await.unwrap();
    assert_eq!(
        read_back.table_details.blueprint_dataset,
        Some(blueprint_dataset)
    );
    assert_eq!(
        read_back.table_details.default_blueprint_segment,
        Some(default_blueprint_segment)
    );
}

pub async fn update_table_entry_rejects_invalid_blueprint_details(service: impl RerunCloudService) {
    let table_dir = tempfile::tempdir().expect("create temp dir");
    let table_entry =
        create_table_entry_with_name(&service, "table_with_invalid_blueprint", &table_dir).await;
    let table_id = table_entry.details.id;
    let implicit_blueprint = table_entry
        .table_details
        .blueprint_dataset
        .expect("tables should get an implicit blueprint dataset");

    let status = update_table_entry(
        &service,
        UpdateTableEntryRequest {
            id: EntryId::new(),
            table_details: TableDetails {
                blueprint_dataset: Some(implicit_blueprint),
                default_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::NotFound,
        "unexpected status: {status:?}"
    );

    let status = update_table_entry(
        &service,
        UpdateTableEntryRequest {
            id: table_id,
            table_details: TableDetails {
                blueprint_dataset: Some(EntryId::new()),
                default_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );

    let recording_dataset = create_dataset_entry(&service, "recording_is_not_table_blueprint")
        .await
        .unwrap();
    let status = update_table_entry(
        &service,
        UpdateTableEntryRequest {
            id: table_id,
            table_details: TableDetails {
                blueprint_dataset: Some(recording_dataset.details.id),
                default_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );
}

pub async fn update_dataset_entry_rejects_invalid_blueprint_details(
    service: impl RerunCloudService,
) {
    let dataset_entry = create_dataset_entry(&service, "dataset_with_blueprint_validation")
        .await
        .unwrap();
    let dataset_id = dataset_entry.details.id;
    let hidden_blueprint = dataset_entry
        .dataset_details
        .blueprint_dataset
        .expect("recording datasets should get an implicit blueprint dataset");

    let hidden_asset = dataset_entry
        .dataset_details
        .asset_dataset
        .expect("recording datasets should get an implicit asset dataset");

    let default_blueprint_segment = SegmentId::from("default_dataset_blueprint");
    let default_segment_table_blueprint_segment =
        SegmentId::from("default_dataset_segment_table_blueprint");

    let updated = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset: Some(hidden_blueprint),
                asset_dataset: Some(hidden_asset),
                default_blueprint_segment: Some(default_blueprint_segment.clone()),
                default_segment_table_blueprint_segment: Some(
                    default_segment_table_blueprint_segment.clone(),
                ),
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(
        updated.dataset_details.blueprint_dataset,
        Some(hidden_blueprint)
    );
    assert_eq!(
        updated.dataset_details.default_blueprint_segment,
        Some(default_blueprint_segment)
    );
    assert_eq!(
        updated
            .dataset_details
            .default_segment_table_blueprint_segment,
        Some(default_segment_table_blueprint_segment)
    );

    let status = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset: None,
                asset_dataset: None,
                default_blueprint_segment: Some(SegmentId::from("missing_blueprint_dataset")),
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );

    let status = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset: Some(EntryId::new()),
                asset_dataset: Some(EntryId::new()),
                default_blueprint_segment: None,
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );

    let recording_dataset = create_dataset_entry(&service, "recording_is_not_dataset_blueprint")
        .await
        .unwrap();
    let status = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset: Some(recording_dataset.details.id),
                asset_dataset: None,
                default_blueprint_segment: None,
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );
}

/// Updating a dataset without providing an asset dataset keeps the existing one. The field is
/// managed by the server, so a client that doesn't know about it cannot accidentally clear it.
pub async fn update_dataset_entry_keeps_asset_dataset(service: impl RerunCloudService) {
    let dataset_entry = create_dataset_entry(&service, "dataset_keeps_asset_dataset")
        .await
        .unwrap();
    let dataset_id = dataset_entry.details.id;
    let hidden_asset = dataset_entry
        .dataset_details
        .asset_dataset
        .expect("recording datasets should get an implicit asset dataset");

    let updated = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset: dataset_entry.dataset_details.blueprint_dataset,
                asset_dataset: None,
                default_blueprint_segment: None,
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(
        updated.dataset_details.asset_dataset,
        Some(hidden_asset),
        "an update without an asset dataset should keep the existing one"
    );
}

/// A dataset whose asset dataset was deleted is left with a dangling reference. Updating the
/// entry replaces the dangling reference with a new asset dataset, whether the client omits the
/// reference or sends the stale one back unchanged.
pub async fn update_dataset_entry_replaces_deleted_asset_dataset(service: impl RerunCloudService) {
    let dataset_entry = create_dataset_entry(&service, "dataset_replaces_deleted_asset")
        .await
        .unwrap();
    let dataset_id = dataset_entry.details.id;
    let blueprint_dataset = dataset_entry.dataset_details.blueprint_dataset;
    let original_asset = dataset_entry
        .dataset_details
        .asset_dataset
        .expect("recording datasets should get an implicit asset dataset");

    // Delete the asset dataset directly, leaving the dataset's reference dangling.
    service
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            id: Some(original_asset.into()),
        }))
        .await
        .expect("failed to delete the asset dataset");

    let updated = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset,
                asset_dataset: None,
                default_blueprint_segment: None,
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap();
    let replacement = updated
        .dataset_details
        .asset_dataset
        .expect("the dangling asset dataset reference should be replaced");
    assert_ne!(replacement, original_asset);

    // A client sending the stored reference back unchanged is not choosing a new one: after
    // deleting the replacement, the stale reference is replaced too.
    service
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            id: Some(replacement.into()),
        }))
        .await
        .expect("failed to delete the replacement asset dataset");
    let updated = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset,
                asset_dataset: Some(replacement),
                default_blueprint_segment: None,
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap();
    let second_replacement = updated
        .dataset_details
        .asset_dataset
        .expect("the stale asset dataset reference should be replaced again");
    assert_ne!(second_replacement, replacement);
}

/// The asset dataset reference of a dataset must point to an existing asset dataset.
pub async fn update_dataset_entry_rejects_invalid_asset_details(service: impl RerunCloudService) {
    let dataset_entry = create_dataset_entry(&service, "dataset_with_asset_validation")
        .await
        .unwrap();
    let dataset_id = dataset_entry.details.id;
    let blueprint_dataset = dataset_entry.dataset_details.blueprint_dataset;

    let status = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset,
                asset_dataset: Some(EntryId::new()),
                default_blueprint_segment: None,
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );

    let recording_dataset = create_dataset_entry(&service, "recording_is_not_asset_dataset")
        .await
        .unwrap();
    let status = update_dataset_entry(
        &service,
        UpdateDatasetEntryRequest {
            id: dataset_id,
            dataset_details: DatasetDetails {
                blueprint_dataset,
                asset_dataset: Some(recording_dataset.details.id),
                default_blueprint_segment: None,
                default_segment_table_blueprint_segment: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );
}

pub async fn update_entry_bumps_timestamp(service: impl RerunCloudService) {
    //
    // Create a dataset
    //

    let dataset_name = "timestamp_test_dataset";
    let dataset_entry = create_dataset_entry(&service, dataset_name).await.unwrap();

    let dataset_id = dataset_entry.details.id;
    let initial_updated_at = dataset_entry.details.updated_at;

    // Small delay to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    //
    // Rename the dataset - this should update the timestamp
    //

    let new_name = "renamed_dataset";
    let response = update_entry(
        &service,
        UpdateEntryRequest {
            id: dataset_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(entry_name(new_name)),
            },
        },
    )
    .await
    .unwrap();

    let after_rename_updated_at = response.entry_details.updated_at;

    assert!(
        after_rename_updated_at > initial_updated_at,
        "Timestamp should be updated after rename. Initial: {initial_updated_at:?}, After rename: {after_rename_updated_at:?}"
    );

    // Small delay to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    //
    // Rename to the same name
    //

    let response2 = update_entry(
        &service,
        UpdateEntryRequest {
            id: dataset_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(entry_name(new_name)),
            },
        },
    )
    .await
    .unwrap();

    let after_second_rename_updated_at = response2.entry_details.updated_at;

    assert_eq!(
        after_second_rename_updated_at, after_rename_updated_at,
        "Timestamp should NOT be updated when renaming to the same name. After first rename: {after_rename_updated_at:?}, After second rename: {after_second_rename_updated_at:?}"
    );
}

// ---

async fn create_dataset_entry(
    service: &impl RerunCloudService,
    name: &str,
) -> tonic::Result<DatasetEntry> {
    service
        .create_dataset_entry(tonic::Request::new(
            CreateDatasetEntryRequest {
                name: entry_name(name),
                id: None,
            }
            .into(),
        ))
        .await
        .map(|result| result.into_inner().dataset.unwrap().try_into().unwrap())
}

async fn update_entry(
    service: &impl RerunCloudService,
    request: UpdateEntryRequest,
) -> tonic::Result<UpdateEntryResponse> {
    service
        .update_entry(tonic::Request::new(request.into()))
        .await
        .map(|response| response.into_inner().try_into().unwrap())
}

async fn update_dataset_entry(
    service: &impl RerunCloudService,
    request: UpdateDatasetEntryRequest,
) -> tonic::Result<DatasetEntry> {
    service
        .update_dataset_entry(tonic::Request::new(request.into()))
        .await
        .map(|response| response.into_inner().dataset.unwrap().try_into().unwrap())
}

async fn update_table_entry(
    service: &impl RerunCloudService,
    request: UpdateTableEntryRequest,
) -> tonic::Result<TableEntry> {
    service
        .update_table_entry(tonic::Request::new(request.into()))
        .await
        .map(|response| response.into_inner().table.unwrap().try_into().unwrap())
}

async fn read_table_entry(
    service: &impl RerunCloudService,
    table_id: EntryId,
) -> tonic::Result<TableEntry> {
    service
        .read_table_entry(tonic::Request::new(ReadTableEntryRequest {
            id: Some(table_id.into()),
        }))
        .await
        .map(|response| response.into_inner().table.unwrap().try_into().unwrap())
}
