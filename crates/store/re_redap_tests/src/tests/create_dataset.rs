use arrow::datatypes::{DataType, Field, Schema};

use re_log_types::EntryId;
use re_protos::{
    cloud::v1alpha1::{
        EntryFilter, EntryKind, FindEntriesRequest, ReadDatasetEntryRequest,
        ext::{
            CreateDatasetEntryRequest, CreateTableEntryRequest, DatasetDetails, DatasetEntry,
            EntryDetails, LanceTable, ProviderDetails as _,
        },
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

pub async fn create_dataset_tests(service: impl RerunCloudService) {
    //
    // Create a dataset with just a name
    //

    let dataset1_name = "dataset1";

    create_dataset_entry(
        &service,
        CreateDatasetEntryRequest {
            name: dataset1_name.to_owned(),
            id: None,
        },
    )
    .await
    .unwrap();

    let entry_details = entry_details_from_name(&service, dataset1_name, EntryKind::Dataset)
        .await
        .unwrap();

    let dataset_details = dataset_details_from_id(&service, entry_details.id)
        .await
        .unwrap();

    assert!(dataset_details.blueprint_dataset.is_some());
    assert!(dataset_details.default_blueprint.is_none());

    //
    // Check the dataset got a matching blueprint dataset
    //

    let bp_entry_id = dataset_details
        .blueprint_dataset
        .expect("there should be a blueprint dataset");

    let _ = entry_details_from_id(&service, bp_entry_id, EntryKind::BlueprintDataset)
        .await
        .unwrap();

    let bp_dataset_details = dataset_details_from_id(&service, bp_entry_id)
        .await
        .unwrap();

    assert!(bp_dataset_details.blueprint_dataset.is_none());
    assert!(bp_dataset_details.default_blueprint.is_none());

    //
    // Check a duplicate entry name is rejected.
    //

    let error_code = create_dataset_entry(
        &service,
        CreateDatasetEntryRequest {
            name: dataset1_name.to_owned(),
            id: None,
        },
    )
    .await
    .unwrap_err()
    .code();

    assert_eq!(error_code, tonic::Code::AlreadyExists);

    //
    // Check a duplicate entry id is rejected.
    //

    let error_code = create_dataset_entry(
        &service,
        CreateDatasetEntryRequest {
            name: "this name is for sure not used, but the id might".to_owned(),
            id: Some(entry_details.id),
        },
    )
    .await
    .unwrap_err()
    .code();

    assert_eq!(error_code, tonic::Code::AlreadyExists);

    //
    // Create another dataset with an enforced entry id
    //

    let dataset2_name = "dataset2";
    let dataset2_id = EntryId::from(re_tuid::Tuid::from_u128(123));

    create_dataset_entry(
        &service,
        CreateDatasetEntryRequest {
            name: dataset2_name.to_owned(),
            id: Some(dataset2_id),
        },
    )
    .await
    .unwrap();

    let _ = entry_details_from_name(&service, dataset2_name, EntryKind::Dataset)
        .await
        .unwrap();

    let _ = entry_details_from_id(&service, dataset2_id, EntryKind::Dataset)
        .await
        .unwrap();

    let dataset_details = dataset_details_from_id(&service, dataset2_id)
        .await
        .unwrap();

    assert!(dataset_details.blueprint_dataset.is_some());
    assert!(dataset_details.default_blueprint.is_none());

    //
    // Create a table
    //

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let table_name = "created_table";
    let schema = Schema::new(vec![Field::new("column_a", DataType::Utf8, false)]);

    let table_url =
        url::Url::from_directory_path(tmp_dir.path()).expect("create url from tmp directory");
    let provider_details = LanceTable { table_url }
        .try_as_any()
        .expect("convert provider details to any");

    let create_table_request = CreateTableEntryRequest {
        name: table_name.to_owned(),
        schema: schema.clone(),
        provider_details,
    }
    .try_into()
    .expect("Unable to create table request");

    let _ = service
        .create_table_entry(tonic::Request::new(create_table_request))
        .await
        .expect("create table entry");

    //
    // Dataset with same name as table fails
    //

    let error_code = create_dataset_entry(
        &service,
        CreateDatasetEntryRequest {
            name: table_name.to_owned(),
            id: None,
        },
    )
    .await
    .unwrap_err()
    .code();

    assert_eq!(error_code, tonic::Code::AlreadyExists);
}

// ---

async fn create_dataset_entry(
    service: &impl RerunCloudService,
    request: CreateDatasetEntryRequest,
) -> Result<DatasetEntry, tonic::Status> {
    service
        .create_dataset_entry(tonic::Request::new(request.clone().into()))
        .await
        .map(|result| result.into_inner().dataset.unwrap().try_into().unwrap())
}

/// Get the entry details or return the endpoint error (all other errors panic)
async fn entry_details_from_name(
    service: &impl RerunCloudService,
    name: &str,
    entry_kind: EntryKind,
) -> Result<EntryDetails, tonic::Status> {
    let mut result = service
        .find_entries(tonic::Request::new(FindEntriesRequest {
            filter: Some(EntryFilter {
                id: None,
                name: Some(name.to_owned()),
                entry_kind: Some(entry_kind as i32),
            }),
        }))
        .await?
        .into_inner()
        .entries;

    assert_eq!(result.len(), 1);

    let entry_details = result.pop().unwrap();
    assert_eq!(entry_details.name.as_deref(), Some(name));
    assert_eq!(entry_details.entry_kind, entry_kind as i32);

    Ok(entry_details.try_into().unwrap())
}

/// Get the entry details or return the endpoint error (all other errors panic)
async fn entry_details_from_id(
    service: &impl RerunCloudService,
    entry_id: EntryId,
    entry_kind: EntryKind,
) -> Result<EntryDetails, tonic::Status> {
    let mut result = service
        .find_entries(tonic::Request::new(FindEntriesRequest {
            filter: Some(EntryFilter {
                id: Some(entry_id.into()),
                name: None,
                entry_kind: Some(entry_kind as i32),
            }),
        }))
        .await?
        .into_inner()
        .entries;

    assert_eq!(result.len(), 1);

    let entry_details = result.pop().unwrap();
    assert_eq!(entry_details.id, Some(entry_id.into()));
    assert_eq!(entry_details.entry_kind, entry_kind as i32);

    Ok(entry_details.try_into().unwrap())
}

/// Get the dataset details or return the endpoint error (all other errors panic)
async fn dataset_details_from_id(
    service: &impl RerunCloudService,
    entry_id: EntryId,
) -> Result<DatasetDetails, tonic::Status> {
    service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {})
                .with_entry_id(entry_id)
                .unwrap(),
        )
        .await
        .map(|resp| {
            resp.into_inner()
                .dataset
                .unwrap()
                .dataset_details
                .unwrap()
                .try_into()
                .unwrap()
        })
}
