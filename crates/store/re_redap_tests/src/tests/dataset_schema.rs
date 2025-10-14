#![allow(clippy::unwrap_used)]

use url::Url;

use re_log_types::EntryId;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, DataSource, DataSourceKind, GetDatasetSchemaRequest,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::tests::common::register_with_dataset_id;
use crate::{SchemaExt as _, create_simple_recording};

pub async fn simple_dataset_schema(fe: impl RerunCloudService) {
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

    register_with_dataset_id(
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

    dataset_schema_snapshot(&fe, dataset_id, "simple_dataset").await;
}

pub async fn empty_dataset_schema(fe: impl RerunCloudService) {
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

    dataset_schema_snapshot(&fe, dataset_id, "empty_dataset").await;
}

// ---

async fn dataset_schema_snapshot(
    fe: &impl RerunCloudService,
    dataset_id: EntryId,
    snapshot_name: &str,
) {
    let schema = fe
        .get_dataset_schema(
            tonic::Request::new(GetDatasetSchemaRequest {})
                .with_entry_id(dataset_id)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .schema()
        .unwrap();

    insta::assert_snapshot!(format!("{snapshot_name}_schema"), schema.format_snapshot());
}
