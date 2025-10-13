#![allow(clippy::unwrap_used)]

use futures::TryStreamExt as _;
use itertools::Itertools as _;

use url::Url;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, DataSource, DataSourceKind, ScanDatasetManifestRequest,
        ScanDatasetManifestResponse, ScanPartitionTableRequest, ScanPartitionTableResponse,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::tests::common::register_with_dataset;
use crate::{RecordBatchExt as _, create_simple_recording};

// We just want to make sure that the dataset resolution logic and the proxy in general both work as
// expected: registering and listing partitions using dataset IDs is a good way to do that.
pub async fn register_and_scan_simple_dataset(fe: impl RerunCloudService) {
    let tuid_prefix1 = 1;
    let partition1_path = create_simple_recording(
        tuid_prefix1,
        "my_partition_id1",
        &["my/entity", "my/other/entity"],
    )
    .unwrap();
    let partition1_url = Url::from_file_path(partition1_path.as_path()).unwrap();

    let tuid_prefix2 = 2;
    let partition2_path =
        create_simple_recording(tuid_prefix2, "my_partition_id2", &["my/entity"]).unwrap();
    let partition2_url = Url::from_file_path(partition2_path.as_path()).unwrap();

    let tuid_prefix3 = 3;
    let partition3_path = create_simple_recording(
        tuid_prefix3,
        "my_partition_id3",
        &["my/entity", "another/one", "yet/another/one"],
    )
    .unwrap();
    let partition3_url = Url::from_file_path(partition3_path.as_path()).unwrap();

    let dataset_id: EntryId = {
        let resp = fe
            .create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
                name: Some("my_dataset1".to_owned()),
                id: None,
            }))
            .await
            .unwrap();

        resp.into_inner()
            .dataset
            .and_then(|d| d.details?.id)
            .unwrap()
            .try_into()
            .unwrap()
    };

    register_with_dataset(
        &fe,
        dataset_id,
        vec![
            DataSource {
                storage_url: Some(partition1_url.to_string()),
                layer: None,
                typ: DataSourceKind::Rrd as i32,
            }, //
            DataSource {
                storage_url: Some(partition2_url.to_string()),
                layer: None,
                typ: DataSourceKind::Rrd as i32,
            },
            DataSource {
                storage_url: Some(partition3_url.to_string()),
                layer: None,
                typ: DataSourceKind::Rrd as i32,
            },
        ],
    )
    .await;

    scan_partition_table_and_snapshot(&fe, dataset_id, "list_all").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_id, "manifest_list_all").await;
}

// Scanning an empty dataset should return an empty dataframe with the expected schema -- not a
// NOT_FOUND error.
pub async fn register_and_scan_empty_dataset(fe: impl RerunCloudService) {
    let dataset_id: EntryId = {
        let resp = fe
            .create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
                name: Some("my_dataset1".to_owned()),
                id: None,
            }))
            .await
            .unwrap();

        resp.into_inner()
            .dataset
            .and_then(|d| d.details?.id)
            .unwrap()
            .try_into()
            .unwrap()
    };

    scan_partition_table_and_snapshot(&fe, dataset_id, "list_empty").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_id, "manifest_list_empty").await;
}

// ---

async fn scan_partition_table_and_snapshot(
    fe: &impl RerunCloudService,
    dataset_id: EntryId,
    snapshot_name: &str,
) {
    let resps: Vec<_> = fe
        .scan_partition_table(
            tonic::Request::new(ScanPartitionTableRequest {
                columns: vec![], // all of them
            })
            .with_entry_id(dataset_id)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    let batches = resps
        .into_iter()
        .map(|resp| resp.data.unwrap().decode().unwrap())
        .collect_vec();

    let batch = arrow::compute::concat_batches(
        batches
            .first()
            .expect("there should be at least one batch")
            .schema_ref(),
        &batches,
    )
    .unwrap()
    .auto_sort_rows()
    .unwrap();

    let columns = ScanPartitionTableResponse::fields();
    let columns_names = columns
        .iter()
        .map(|field| field.name().as_str())
        .filter(|name| {
            // these are implementation-dependent
            name != &ScanPartitionTableResponse::FIELD_STORAGE_URLS
                && name != &ScanPartitionTableResponse::FIELD_SIZE_BYTES
                // these are unstable
                && name != &ScanPartitionTableResponse::FIELD_LAST_UPDATED_AT
        })
        .collect_vec();
    let filtered_batch = batch.filtered_columns(&columns_names);

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_batch.format_snapshot(false)
    );
}

async fn scan_dataset_manifest_and_snapshot(
    fe: &impl RerunCloudService,
    dataset_id: EntryId,
    snapshot_name: &str,
) {
    let response = fe
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest {
                columns: vec![], // all of them
            })
            .with_entry_id(dataset_id)
            .unwrap(),
        )
        .await;

    //TODO(RR-2482): remove this once OSS server implements this endpoint
    if response
        .as_ref()
        .is_err_and(|status| status.code() == tonic::Code::Unimplemented)
    {
        return;
    }

    let resps: Vec<_> = response.unwrap().into_inner().try_collect().await.unwrap();

    let batches = resps
        .into_iter()
        .map(|resp| resp.data.unwrap().decode().unwrap())
        .collect_vec();

    let batch = arrow::compute::concat_batches(
        batches
            .first()
            .expect("there should be at least one batch")
            .schema_ref(),
        &batches,
    )
    .unwrap();

    let columns = ScanDatasetManifestResponse::fields();
    let columns_names = columns
        .iter()
        .map(|field| field.name().as_str())
        .filter(|name| {
            // these are implementation-dependent
            name != &ScanDatasetManifestResponse::FIELD_STORAGE_URL
                && name != &ScanDatasetManifestResponse::FIELD_SIZE_BYTES
                // these are unstable
                && name != &ScanDatasetManifestResponse::FIELD_LAST_UPDATED_AT
                && name != &ScanDatasetManifestResponse::FIELD_REGISTRATION_TIME
        })
        .collect_vec();
    let filtered_batch = batch
        .filtered_columns(&columns_names)
        .auto_sort_rows()
        .unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_batch.format_snapshot(false)
    );
}
