#![expect(clippy::unwrap_used)]

use futures::TryStreamExt as _;
use itertools::Itertools as _;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, ScanDatasetManifestRequest, ScanDatasetManifestResponse,
        ScanPartitionTableRequest, ScanPartitionTableResponse,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use super::common::{DataSourcesDefinition, LayerDefinition, register_with_dataset_name};
use crate::RecordBatchExt as _;

pub async fn register_and_scan_simple_dataset(fe: impl RerunCloudService) {
    let mut data_sources_def = DataSourcesDefinition::new([
        LayerDefinition {
            partition_id: "my_partition_id1",
            layer_name: None,
            entity_paths: &["my/entity", "my/other/entity"],
        },
        LayerDefinition {
            partition_id: "my_partition_id2",
            layer_name: None,
            entity_paths: &["my/entity"],
        },
        LayerDefinition {
            partition_id: "my_partition_id3",
            layer_name: None,
            entity_paths: &["my/entity", "another/one", "yet/another/one"],
        },
    ]);

    data_sources_def.generate_simple();

    let dataset_name = "my_dataset1";
    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .unwrap();

    register_with_dataset_name(&fe, dataset_name, data_sources_def.to_data_sources()).await;

    scan_partition_table_and_snapshot(&fe, dataset_name, "simple").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_name, "simple").await;
}

pub async fn register_and_scan_simple_dataset_with_layers(fe: impl RerunCloudService) {
    let mut data_sources_def = DataSourcesDefinition::new([
        LayerDefinition {
            partition_id: "partition1",
            layer_name: None,
            entity_paths: &["my/entity", "another/one", "yet/another/one"],
        },
        LayerDefinition {
            partition_id: "partition1",
            layer_name: Some("extra"),
            entity_paths: &["extra/entity"],
        },
        LayerDefinition {
            partition_id: "partition2",
            layer_name: Some("base"),
            entity_paths: &["another/one", "yet/another/one"],
        },
        LayerDefinition {
            partition_id: "partition2",
            layer_name: Some("extra"),
            entity_paths: &["extra/entity"],
        },
        LayerDefinition {
            partition_id: "partition3",
            layer_name: None,
            entity_paths: &["i/am/alone"],
        },
    ]);

    data_sources_def.generate_simple();

    let dataset_name = "dataset_with_layers";
    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .unwrap();

    register_with_dataset_name(&fe, dataset_name, data_sources_def.to_data_sources()).await;

    scan_partition_table_and_snapshot(&fe, dataset_name, "simple_with_layers").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_name, "simple_with_layers").await;
}

// Scanning an empty dataset should return an empty dataframe with the expected schema -- not a
// NOT_FOUND error.
pub async fn register_and_scan_empty_dataset(fe: impl RerunCloudService) {
    let dataset_name = "empty_dataset";
    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .unwrap();

    scan_partition_table_and_snapshot(&fe, dataset_name, "empty").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_name, "empty").await;
}

// ---

async fn scan_partition_table_and_snapshot(
    fe: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
) {
    let resps: Vec<_> = fe
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
        format!("{snapshot_name}_partitions_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_partitions_data"),
        filtered_batch.format_snapshot(false)
    );
}

async fn scan_dataset_manifest_and_snapshot(
    fe: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
) {
    let resps: Vec<_> = fe
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
        format!("{snapshot_name}_manifest_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_manifest_data"),
        filtered_batch.format_snapshot(false)
    );
}
