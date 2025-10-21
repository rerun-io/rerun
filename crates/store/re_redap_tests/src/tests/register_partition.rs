#![expect(clippy::unwrap_used)]

use arrow::array::RecordBatch;
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use url::Url;

use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, DataSource, DataSourceKind, ScanDatasetManifestRequest,
        ScanDatasetManifestResponse, ScanPartitionTableRequest, ScanPartitionTableResponse,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use super::common::{DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _};
use crate::{RecordBatchExt as _, create_simple_recording_in};

pub async fn register_and_scan_simple_dataset(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new([
        LayerDefinition::simple("my_partition_id1", &["my/entity", "my/other/entity"]),
        LayerDefinition::simple("my_partition_id2", &["my/entity"]),
        LayerDefinition::simple(
            "my_partition_id3",
            &["my/entity", "another/one", "yet/another/one"],
        ),
    ]);

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name(dataset_name, data_sources_def.to_data_sources())
        .await;

    scan_partition_table_and_snapshot(&service, dataset_name, "simple").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "simple").await;
}

pub async fn register_and_scan_simple_dataset_with_layers(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new([
        LayerDefinition::simple(
            "partition1",
            &["my/entity", "another/one", "yet/another/one"],
        ),
        LayerDefinition::simple("partition1", &["extra/entity"]).layer_name("extra"),
        LayerDefinition::simple("partition2", &["another/one", "yet/another/one"])
            .layer_name("base"),
        LayerDefinition::simple("partition2", &["extra/entity"]).layer_name("extra"),
        LayerDefinition::simple("partition3", &["i/am/alone"]),
    ]);

    let dataset_name = "dataset_with_layers";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name(dataset_name, data_sources_def.to_data_sources())
        .await;

    scan_partition_table_and_snapshot(&service, dataset_name, "simple_with_layers").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "simple_with_layers").await;
}

pub async fn register_with_prefix(fe: impl RerunCloudService) {
    let root_dir = tempfile::tempdir().expect("creating temp dir");

    // Note: for this test, we don't use teh `DataSourceDefinition` abstraction here because we need
    // tight control of where the RRDs are stored.
    let tuid_prefix1 = 1;
    create_simple_recording_in(
        tuid_prefix1,
        "my_partition_id1",
        &["my/entity", "my/other/entity"],
        root_dir.path(),
    )
    .expect("creating recording");

    let tuid_prefix2 = 2;
    create_simple_recording_in(
        tuid_prefix2,
        "my_partition_id2",
        &["my/entity"],
        root_dir.path(),
    )
    .expect("creating recording");

    let tuid_prefix3 = 3;
    create_simple_recording_in(
        tuid_prefix3,
        "my_partition_id3",
        &["my/entity", "another/one", "yet/another/one"],
        root_dir.path(),
    )
    .expect("creating recording");

    let dataset_name = "my_dataset1";
    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .unwrap();

    let root_url =
        Url::parse(&format!("file://{}/", root_dir.path().display())).expect("creating root url");

    fe.register_with_dataset_name(
        dataset_name,
        vec![
            DataSource {
                storage_url: Some(root_url.to_string()),
                prefix: true,
                layer: None,
                typ: DataSourceKind::Rrd as i32,
            }, //
        ],
    )
    .await;

    scan_partition_table_and_snapshot(&fe, dataset_name, "register_prefix_partitions").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_name, "register_prefix_manifest").await;
}

// Scanning an empty dataset should return an empty dataframe with the expected schema -- not a
// NOT_FOUND error.
pub async fn register_and_scan_empty_dataset(service: impl RerunCloudService) {
    let dataset_name = "empty_dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    scan_partition_table_and_snapshot(&service, dataset_name, "empty").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "empty").await;
}

// ---

async fn scan_partition_table_and_snapshot(
    service: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
) {
    let resps: Vec<_> = service
        .scan_partition_table(
            tonic::Request::new(ScanPartitionTableRequest {
                columns: vec![], // all of them
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    let batches: Vec<RecordBatch> = resps
        .into_iter()
        .map(|resp| resp.data.unwrap().try_into().unwrap())
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
        format!("{snapshot_name}_partitions_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_partitions_data"),
        filtered_batch.format_snapshot(false)
    );
}

async fn scan_dataset_manifest_and_snapshot(
    service: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
) {
    let resps: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest {
                columns: vec![], // all of them
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    let batches: Vec<RecordBatch> = resps
        .into_iter()
        .map(|resp| resp.data.unwrap().try_into().unwrap())
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
        format!("{snapshot_name}_manifest_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_manifest_data"),
        filtered_batch.format_snapshot(false)
    );
}
