use futures::StreamExt as _;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, QueryDatasetResponse, ext::QueryDatasetRequest,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::RecordBatchExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, concat_record_batches, register_with_dataset_name,
};

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

    let dataset_name = "dataset";

    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .expect("Failed to create dataset");

    // now register partitions with the dataset
    register_with_dataset_name(&fe, dataset_name, data_sources_def.to_data_sources()).await;

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

    let merged_chunk_info = concat_record_batches(&chunk_info);

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
