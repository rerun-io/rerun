use crate::tests::common::RerunCloudServiceExt as _;
use crate::utils::streaming::make_streaming_request;
use crate::utils::tables::create_simple_lance_dataset;
use arrow::array::RecordBatch;
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_protos::cloud::v1alpha1::TableInsertMode;
use re_protos::cloud::v1alpha1::{
    FindEntriesRequest, ScanTableRequest, WriteTableRequest, ext::EntryDetails,
    rerun_cloud_service_server::RerunCloudService,
};
use re_protos::headers::RerunHeadersInjectorExt as _;

async fn get_table_batches(
    service: &impl RerunCloudService,
    entry: &EntryDetails,
) -> Vec<RecordBatch> {
    let scan_request = ScanTableRequest {
        table_id: Some(entry.id.into()),
    };

    let table_resp: Vec<_> = service
        .scan_table(tonic::Request::new(scan_request))
        .await
        .expect("Failed to scan table")
        .into_inner()
        .try_collect()
        .await
        .expect("Failed to collect scan results");

    table_resp
        .into_iter()
        .map(|resp| {
            resp.dataframe_part
                .expect("Expected dataframe part")
                .try_into()
                .expect("Failed to decode dataframe")
        })
        .collect_vec()
}

pub async fn write_table(service: impl RerunCloudService) {
    let table_name = "test_table";
    let path = create_simple_lance_dataset()
        .await
        .expect("Unable to create lance dataset");

    service
        .register_table_with_name(table_name, path.as_path())
        .await;

    let find_table = FindEntriesRequest {
        filter: Some(re_protos::cloud::v1alpha1::EntryFilter {
            name: Some(table_name.to_owned()),
            ..Default::default()
        }),
    };

    let table_entry_resp = service
        .find_entries(tonic::Request::new(find_table))
        .await
        .expect("Failed to find entries")
        .into_inner()
        .entries;

    assert_eq!(table_entry_resp.len(), 1);

    let entry: EntryDetails = table_entry_resp[0]
        .clone()
        .try_into()
        .expect("Failed to convert to EntryDetails");

    assert_eq!(entry.name, table_name);

    let original_batches = get_table_batches(&service, &entry).await;
    assert_ne!(original_batches.len(), 0);

    let original_rows: usize = original_batches.iter().map(|batch| batch.num_rows()).sum();
    assert_ne!(original_rows, 0); // Make sure we have some data or the below checks do not make sense

    let append_batches = original_batches
        .iter()
        .map(|batch| WriteTableRequest {
            dataframe_part: Some(batch.into()),
            insert_mode: TableInsertMode::Append.into(),
        })
        .collect_vec();

    service
        .write_table(
            make_streaming_request(append_batches)
                .with_entry_id(entry.id)
                .expect("Unable to set entry_id on write table"),
        )
        .await
        .expect("Failed to write table in append mode");

    // Verify that we have doubled the size of our table
    // since we basically wrote back everything that was in it
    // in append mode

    let returned_batches = get_table_batches(&service, &entry).await;
    let returned_rows: usize = returned_batches.iter().map(|batch| batch.num_rows()).sum();
    assert_eq!(returned_rows, 2 * original_rows);

    let overwrite_batches = original_batches
        .iter()
        .map(|batch| WriteTableRequest {
            dataframe_part: Some(batch.into()),
            insert_mode: TableInsertMode::Overwrite.into(),
        })
        .collect_vec();

    service
        .write_table(
            make_streaming_request(overwrite_batches)
                .with_entry_id(entry.id)
                .expect("Unable to set entry_id on write table"),
        )
        .await
        .expect("Failed to write table in overwrite");

    // Verify that we have essentially reverted the append because
    // our new overwrite has the same data as our original table

    let returned_batches = get_table_batches(&service, &entry).await;
    let returned_rows: usize = returned_batches.iter().map(|batch| batch.num_rows()).sum();
    assert_eq!(returned_rows, original_rows);
}
