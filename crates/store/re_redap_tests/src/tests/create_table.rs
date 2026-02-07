use std::collections::HashMap;

use arrow::datatypes::{DataType, Field, Schema};
use re_protos::cloud::v1alpha1::GetTableSchemaRequest;
use re_protos::cloud::v1alpha1::ext::{
    CreateTableEntryRequest, EntryDetails, LanceTable, ProviderDetails,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;

use crate::SchemaTestExt as _;

pub async fn create_table_entry(service: impl RerunCloudService) {
    let tmp_dir = tempfile::tempdir().expect("create temp dir");

    let table_name = "created_table";

    let schema = Schema::new(vec![
        Field::new("column_a", DataType::Utf8, false),
        Field::new("column_b", DataType::Int64, true),
        Field::new("column_c", DataType::Float64, false),
        Field::new("column_d", DataType::Boolean, true),
    ]);

    let table_url =
        url::Url::from_directory_path(tmp_dir.path()).expect("create url from tmp directory");
    let provider_details = ProviderDetails::LanceTable(LanceTable { table_url });

    let create_table_request = CreateTableEntryRequest {
        name: table_name.to_owned(),
        schema: schema.clone(),
        provider_details: Some(provider_details),
    }
    .try_into()
    .expect("Unable to create table request");

    let response = service
        .create_table_entry(tonic::Request::new(create_table_request))
        .await
        .expect("create table entry");

    let response = response
        .into_inner()
        .table
        .expect("table missing in create_table response")
        .details
        .expect("table entry details missing");
    let entry: EntryDetails = response.try_into().expect("convert into entry details");

    assert_eq!(entry.name, table_name);

    let schema_response = service
        .get_table_schema(tonic::Request::new(GetTableSchemaRequest {
            table_id: Some(entry.id.into()),
        }))
        .await
        .expect("get table schema")
        .into_inner();

    let returned_schema: Schema = schema_response
        .schema
        .expect("schema is not set in response")
        .try_into()
        .expect("Unable to convert into schema");

    // Strip the metadata that gets added during creation
    let returned_schema = returned_schema.with_metadata(HashMap::new());

    assert_eq!(schema, returned_schema);

    insta::assert_snapshot!("create_table_data", returned_schema.format_snapshot());
}
