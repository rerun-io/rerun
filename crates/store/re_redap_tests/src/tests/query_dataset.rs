use arrow::datatypes::Schema;
use futures::StreamExt as _;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    cloud::v1alpha1::{
        QueryDatasetResponse, ext::QueryDatasetRequest,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::RecordBatchExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
};

pub async fn query_empty_dataset(service: impl RerunCloudService) {
    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    query_dataset_snapshot(
        &service,
        QueryDatasetRequest::default(),
        dataset_name,
        "empty_dataset",
    )
    .await;
}

pub async fn query_simple_dataset(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new([
        LayerDefinition::simple("my_partition_id1", &["my/entity", "my/other/entity"]),
        LayerDefinition::simple("my_partition_id2", &["my/entity"]),
        LayerDefinition::simple(
            "my_partition_id3",
            &["my/entity", "another/one", "yet/another/one"],
        ),
    ]);

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name(dataset_name, data_sources_def.to_data_sources())
        .await;

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
            &service,
            request,
            dataset_name,
            &format!("simple_dataset_{snapshot_name}"),
        )
        .await;
    }
}

/// Test that failure cases return the correct error code.
pub async fn query_dataset_should_fail(service: impl RerunCloudService) {
    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

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
        let response = service
            .query_dataset(tonic::Request::new(request.into()))
            .await;

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
    service: &impl RerunCloudService,
    query_dataset_request: QueryDatasetRequest,
    dataset_name: &str,
    snapshot_name: &str,
) {
    let chunk_info = service
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

    // these are the only columns guaranteed to be returned by `query_dataset`
    let required_field = QueryDatasetResponse::fields();

    assert!(
        merged_chunk_info
            .schema()
            .contains(&Schema::new_with_metadata(
                required_field.clone(),
                Default::default()
            )),
        "query dataset must return all guaranteed fields\nExpected: {:#?}\nGot: {:#?}",
        &required_field,
        merged_chunk_info.schema()
    );

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
