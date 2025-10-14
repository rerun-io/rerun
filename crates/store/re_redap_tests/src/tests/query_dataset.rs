use futures::StreamExt as _;
use url::Url;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, QueryDatasetResponse, ext::QueryDatasetRequest,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt,
};

use crate::tests::common;
use crate::{RecordBatchExt as _, create_simple_recording};

pub async fn query_empty_dataset(fe: impl RerunCloudService) {
    let dataset_name = "dataset";

    fe.create_dataset_entry(tonic::Request::new(
        re_protos::cloud::v1alpha1::CreateDatasetEntryRequest {
            name: Some(dataset_name.to_owned()),
            id: None,
        },
    ))
    .await
    .expect("Failed to create dataset");

    query_dataset_snapshot(
        &fe,
        QueryDatasetRequest::default(),
        dataset_name,
        "empty_dataset",
    )
    .await;
}

pub async fn query_simple_dataset(fe: impl RerunCloudService) {
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

    let partitions = vec![
        common::rrd_datasource(partition1_url),
        common::rrd_datasource(partition2_url),
        common::rrd_datasource(partition3_url),
    ];

    let dataset_name = "dataset";

    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .expect("Failed to create dataset");

    // now register partitions with the dataset
    common::register_with_dataset_name(&fe, dataset_name, partitions).await;

    query_dataset_snapshot(
        &fe,
        QueryDatasetRequest::default(),
        dataset_name,
        "simple_dataset1",
    )
    .await;
}

// ---

async fn query_dataset_snapshot(
    fe: &impl RerunCloudService,
    query_dataset_request: QueryDatasetRequest,
    dataset_name: &str,
    snapshot_name: &str,
) {
    let chunk_info = fe
        .query_dataset(
            tonic::Request::new(query_dataset_request.into())
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().data))
        .map(|dfp| dfp.decode().unwrap())
        .collect::<Vec<_>>()
        .await;

    let merged_chunk_info = common::concat_record_batches(chunk_info);

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        merged_chunk_info.format_schema_snapshot()
    );

    let filtered_chunk_info =
        merged_chunk_info.unfiltered_columns(&[QueryDatasetResponse::FIELD_CHUNK_KEY]);

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_chunk_info.format_snapshot(false)
    );
}
