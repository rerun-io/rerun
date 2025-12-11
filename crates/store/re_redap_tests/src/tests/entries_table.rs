use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::EntryDetails;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    DeleteEntryRequest, FindEntriesRequest, GetTableSchemaRequest, ScanTableRequest,
};

use crate::tests::common::RerunCloudServiceExt as _;
use crate::{RecordBatchTestExt as _, SchemaTestExt as _};

/// We want to make sure that the "__entries" table is present and has the expected schema and data.
pub async fn list_entries_table(service: impl RerunCloudService) {
    let entries_table_id = entries_table_id(&service).await;

    let schema_request = GetTableSchemaRequest {
        table_id: Some(entries_table_id.into()),
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

    insta::assert_snapshot!("entries_table_schema", schema.format_snapshot());

    let scan_request = ScanTableRequest {
        table_id: Some(entries_table_id.into()),
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
                .try_into()
                .expect("Failed to decode dataframe")
        })
        .collect_vec();

    let batch =
        re_arrow_util::concat_polymorphic_batches(&batches).expect("Failed to concat batches");

    assert_eq!(batch.schema().fields(), schema.fields());

    let batch = batch.project_columns(&["name", "entry_kind"]);

    insta::assert_snapshot!("entries_table_data", batch.format_snapshot(false));
}

pub async fn entries_table_with_empty_dataset(service: impl RerunCloudService) {
    let dataset_name = "empty_dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;

    snapshot_entries_table(&service, "entries_table_with_empty_dataset").await;

    service
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            id: Some(dataset_entry.details.id.into()),
        }))
        .await
        .expect("Failed to delete entry");

    snapshot_entries_table(&service, "entries_table_with_empty_dataset_deleted").await;
}

async fn entries_table_id(service: &impl RerunCloudService) -> EntryId {
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

    assert_eq!(entries_resp.len(), 1);

    let entries: EntryDetails = entries_resp[0]
        .clone()
        .try_into()
        .expect("Failed to convert to EntryDetails");

    assert_eq!(entries.name, "__entries");

    entries.id
}

async fn snapshot_entries_table(service: &impl RerunCloudService, snapshot_name: &str) {
    let entries_table_id = entries_table_id(service).await;

    let entries_resp: Vec<_> = service
        .scan_table(tonic::Request::new(ScanTableRequest {
            table_id: Some(entries_table_id.into()),
        }))
        .await
        .expect("Failed to scan table")
        .into_inner()
        .try_collect()
        .await
        .expect("Failed to collect scan results");

    let batches = entries_resp
        .into_iter()
        .map(|resp| {
            resp.dataframe_part
                .expect("Expected dataframe part")
                .try_into()
                .expect("Failed to decode dataframe")
        })
        .collect_vec();

    let batch =
        re_arrow_util::concat_polymorphic_batches(&batches).expect("Failed to concat batches");

    let batch = batch
        .project_columns(&["name", "entry_kind"])
        .auto_sort_rows()
        .unwrap();

    let mut settings = insta::Settings::clone_current();
    settings.add_filter(
        r"__bp_[0-9a-fA-F]{32}",
        "__bp_********************************",
    );
    settings.bind(|| {
        insta::assert_snapshot!(snapshot_name, batch.format_snapshot(false));
    });
}
