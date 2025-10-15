#![expect(clippy::unwrap_used)]

use futures::TryStreamExt as _;
use itertools::{Itertools as _, multizip};

use url::Url;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, DataSource, DataSourceKind, ScanDatasetManifestRequest,
        ScanDatasetManifestResponse, ScanPartitionTableRequest, ScanPartitionTableResponse,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::tests::common::register_with_dataset_name;
use crate::{RecordBatchExt as _, create_simple_recording};

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

    let dataset_name = "my_dataset1";
    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .unwrap();

    register_with_dataset_name(
        &fe,
        dataset_name,
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

    scan_partition_table_and_snapshot(&fe, dataset_name, "simple").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_name, "simple").await;
}

pub async fn register_and_scan_simple_dataset_with_layers(fe: impl RerunCloudService) {
    struct LayerDefinition {
        partition_id: &'static str,
        layer_name: Option<&'static str>,
        entity_paths: &'static [&'static str],
    }

    let layers_to_register = vec![
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
    ];

    let paths = layers_to_register
        .iter()
        .enumerate()
        .map(|(tuid_prefix, l)| {
            create_simple_recording(tuid_prefix as _, l.partition_id, l.entity_paths).unwrap()
        })
        .collect_vec();

    let data_sources = multizip((&layers_to_register, &paths))
        .map(|(l, p)| DataSource {
            storage_url: Some(Url::from_file_path(p.as_path()).unwrap().to_string()),
            layer: l.layer_name.map(|l| l.to_owned()),
            typ: DataSourceKind::Rrd as i32,
        })
        .collect_vec();

    let dataset_name = "dataset_with_layers";
    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .unwrap();

    register_with_dataset_name(&fe, dataset_name, data_sources).await;

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
