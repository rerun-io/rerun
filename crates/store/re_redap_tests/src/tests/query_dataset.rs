use futures::StreamExt as _;
use url::Url;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, QueryDatasetResponse, ext::QueryDatasetRequest,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
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

    let requests = vec![
        (QueryDatasetRequest::default(), "default"),
        (
            QueryDatasetRequest {
                partition_ids: vec!["my_partition_id3".into()],
                ..Default::default()
            },
            "single_partition",
        ),
        (
            QueryDatasetRequest {
                entity_paths: vec!["/my/entity".into()],
                select_all_entity_paths: false,
                ..Default::default()
            },
            "single_entity",
        ),
        //TODO(RR-2613): add more test cases here when they are supported by OSS server
    ];

    for (request, snapshot_name) in requests {
        query_dataset_snapshot(
            &fe,
            request,
            dataset_name,
            &format!("simple_dataset_{snapshot_name}"),
        )
        .await;
    }
}

/// Test that failure cases return the correct error code.
pub async fn query_dataset_should_fail(fe: impl RerunCloudService) {
    let dataset_name = "dataset";

    fe.create_dataset_entry(tonic::Request::new(
        re_protos::cloud::v1alpha1::CreateDatasetEntryRequest {
            name: Some(dataset_name.to_owned()),
            id: None,
        },
    ))
    .await
    .expect("Failed to create dataset");

    let test_cases = vec![
        (
            "cannot specify entity paths if `select_all_entity_paths` is true",
            QueryDatasetRequest {
                entity_paths: vec!["/entity/path".into()],
                select_all_entity_paths: true,
                ..Default::default()
            },
            tonic::Code::InvalidArgument,
        ),
        //TODO(ab): add more failure cases
    ];

    for (descr, request, expected_code) in test_cases {
        let response = fe.query_dataset(tonic::Request::new(request.into())).await;

        match response {
            Ok(_) => {
                panic!("expected failure with code {expected_code}, but got success ({descr})",);
            }
            Err(err) => {
                assert_eq!(
                    err.code(),
                    expected_code,
                    "expected failure with code {expected_code}, but got {err} ({descr})"
                );
            }
        }
    }
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

    let merged_chunk_info = common::concat_record_batches(&chunk_info);

    // these are the only required columns
    let required_field = QueryDatasetResponse::fields();
    let required_column_names = required_field
        .iter()
        .map(|f| f.name().as_str())
        .collect::<Vec<_>>();
    let required_chunk_info = merged_chunk_info.filtered_columns(&required_column_names);

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        required_chunk_info.format_schema_snapshot()
    );

    // these columns are not stable, so we cannot snapshot them
    let filtered_chunk_info = required_chunk_info
        .unfiltered_columns(&[QueryDatasetResponse::FIELD_CHUNK_KEY])
        .auto_sort_rows()
        .unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_chunk_info.format_snapshot(false)
    );
}
