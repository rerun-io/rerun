use arrow::datatypes::{DataType, Field, Schema};
use re_protos::cloud::v1alpha1::ext::{
    CreateDatasetEntryRequest, CreateTableEntryRequest, CreateTableEntryResponse, DatasetEntry,
    EntryDetailsUpdate, LanceTable, ProviderDetails, TableEntry, UpdateEntryRequest,
    UpdateEntryResponse,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;

pub async fn update_entry_tests(service: impl RerunCloudService) {
    //
    // Create a dataset
    //

    let dataset_name = "initial_name";

    let dataset_entry = create_dataset_entry(&service, dataset_name).await.unwrap();

    assert_eq!(dataset_entry.details.name, dataset_name);

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

    assert_eq!(response.entry_details.name, dataset_name);

    //
    // Dataset rename should succeed
    //

    let new_dataset_name = "new_name";
    let response = update_entry(
        &service,
        UpdateEntryRequest {
            id: dataset_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(new_dataset_name.to_owned()),
            },
        },
    )
    .await
    .unwrap();

    assert_eq!(response.entry_details.name, new_dataset_name);

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
                name: Some(new_dataset_name.to_owned()),
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

    let table_entry = create_table_entry(&service, table_name, &table_dir)
        .await
        .unwrap();

    assert_eq!(table_entry.details.name, table_name);
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
                name: Some(new_table_name.to_owned()),
            },
        },
    )
    .await
    .unwrap();

    assert_eq!(response.entry_details.name, new_table_name);

    //
    // Updating table name to an existing dataset name should fail.
    //

    let status = update_entry(
        &service,
        UpdateEntryRequest {
            id: table_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(dataset2_name.to_owned()),
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

    let table2_entry = create_table_entry(&service, table2_name, &table2_dir)
        .await
        .unwrap();
    let table2_id = table2_entry.details.id;

    //
    // Rename to an existing table name should fail.
    //

    let status = update_entry(
        &service,
        UpdateEntryRequest {
            id: table2_id,
            entry_details_update: EntryDetailsUpdate {
                name: Some(new_table_name.to_owned()),
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
                name: Some(new_name.to_owned()),
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
                name: Some(new_name.to_owned()),
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
                name: name.to_owned(),
                id: None,
            }
            .into(),
        ))
        .await
        .map(|result| result.into_inner().dataset.unwrap().try_into().unwrap())
}

async fn create_table_entry(
    service: &impl RerunCloudService,
    table_name: &str,
    tmp_dir: &tempfile::TempDir,
) -> tonic::Result<TableEntry> {
    let schema = Schema::new(vec![Field::new("column_a", DataType::Utf8, false)]);

    let table_url =
        url::Url::from_directory_path(tmp_dir.path()).expect("create url from tmp directory");
    let provider_details = ProviderDetails::LanceTable(LanceTable { table_url });

    service
        .create_table_entry(tonic::Request::new(
            CreateTableEntryRequest {
                name: table_name.to_owned(),
                schema: schema.clone(),
                provider_details: Some(provider_details),
            }
            .try_into()
            .unwrap(),
        ))
        .await
        .map(|result| {
            let resp: CreateTableEntryResponse = result.into_inner().try_into().unwrap();
            resp.table
        })
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
