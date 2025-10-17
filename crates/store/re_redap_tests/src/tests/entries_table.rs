use futures::TryStreamExt as _;
use itertools::Itertools as _;

use re_protos::cloud::v1alpha1::{
    FindEntriesRequest, GetTableSchemaRequest, ScanTableRequest, ext::EntryDetails,
    rerun_cloud_service_server::RerunCloudService,
};
use re_sdk::external::re_log_encoding::codec::wire::decoder::Decode as _;

use crate::{RecordBatchExt as _, SchemaExt as _};

/// We want to make sure that the "__entries" table is present and has the expected schema and data.
pub async fn list_entries_table(service: impl RerunCloudService) {
    let find_entries_table = FindEntriesRequest {
        filter: Some(re_protos::cloud::v1alpha1::EntryFilter {
            name: Some("__entries".to_owned()),
            ..Default::default()
        }),
    };

    let entries_resp = service
        .find_entries(tonic::Request::new(find_entries_table))
        .await
        .expect("Failed to find entries")
        .into_inner()
        .entries;

    assert!(entries_resp.len() == 1);

    let entries: EntryDetails = entries_resp[0]
        .clone()
        .try_into()
        .expect("Failed to convert to EntryDetails");

    assert!(entries.name == "__entries");

    let schema_request = GetTableSchemaRequest {
        table_id: Some(entries.id.into()),
    };

    let schema: arrow::datatypes::Schema = (&service
        .get_table_schema(tonic::Request::new(schema_request))
        .await
        .expect("Failed to get table schema")
        .into_inner()
        .schema
        .expect("Schema should be present"))
        .try_into()
        .expect("Failed to convert schema");

    insta::assert_snapshot!(format!("entries_table_schema"), schema.format_snapshot());

    let scan_request = ScanTableRequest {
        table_id: Some(entries.id.into()),
    };

    let table_resp: Vec<_> = service
        .scan_table(tonic::Request::new(scan_request))
        .await
        .expect("Failed to scan table")
        .into_inner()
        .try_collect()
        .await
        .expect("Failed to collect scan results");

    let batches = table_resp
        .into_iter()
        .map(|resp| {
            resp.dataframe_part
                .expect("Expected dataframe part")
                .decode()
                .expect("Failed to decode dataframe")
        })
        .collect_vec();

    let batch =
        re_arrow_util::concat_polymorphic_batches(&batches).expect("Failed to concat batches");

    assert_eq!(batch.schema().fields(), schema.fields());

    let batch = batch.filtered_columns(&["name", "entry_kind"]);

    insta::assert_snapshot!(format!("entries_table_data"), batch.format_snapshot(false));
}
