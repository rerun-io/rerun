use std::collections::HashSet;

use futures::StreamExt as _;
use itertools::Itertools as _;
use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{FetchChunksRequest, QueryDatasetResponse};
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_sdk::external::re_log_encoding::ToApplication as _;
use re_tuid::Tuid;
use re_types_core::Loggable as _;

use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
};

/// This test makes a snapshot of all the chunks returned for a simple dataset.
///
/// In general, there is no guarantee made on the chunk representation of the underlying data (aka
/// chunks can be split/compacted/etc. arbitrarily by the implementation). So this test is
/// conceptually incorrect and works only because the data/chunk layout used is very basic and
/// predictable.
pub async fn simple_dataset_fetch_chunk_snapshot(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("my_segment_id1", &["my/entity", "my/other/entity"]),
            LayerDefinition::simple("my_segment_id2", &["my/entity"]),
            LayerDefinition::simple(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
        ],
    );

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let chunk_info = service
        .query_dataset(
            tonic::Request::new(QueryDatasetRequest::default().into())
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().data))
        .map(|dfp| dfp.try_into().unwrap())
        .collect::<Vec<_>>()
        .await;

    let required_columns = FetchChunksRequest::required_column_names();
    let required_columns_ref = required_columns.iter().map(|s| s.as_str()).collect_vec();
    let chunk_keys = concat_record_batches(&chunk_info)
        .sort_rows_by(&[QueryDatasetResponse::FIELD_CHUNK_ID])
        .unwrap()
        .project_columns(&required_columns_ref);

    let mut chunks = service
        .fetch_chunks(tonic::Request::new(FetchChunksRequest {
            chunk_infos: vec![chunk_keys.into()],
        }))
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().chunks))
        .map(|msg| re_chunk::Chunk::from_arrow_msg(&msg.to_application(()).unwrap()))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // IMPORTANT: `FetchChunks` does not guarantee chunk ordering
    chunks.sort_by_key(|chunk| chunk.id());

    let printed = chunks.iter().map(|chunk| format!("{chunk:240}")).join("\n");

    insta::assert_snapshot!("simple_dataset_fetch_chunk", printed);
}

/// This test runs a `FetchChunks` spanning multiple datasets and ensures all requested chunks
/// are successfully returned.
pub async fn multi_dataset_fetch_chunk_completeness(service: impl RerunCloudService) {
    //
    // Create first dataset
    //

    let data_sources_def_1 = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("my_segment_id1", &["my/entity", "my/other/entity"]),
            LayerDefinition::simple("my_segment_id2", &["my/entity"]),
            LayerDefinition::simple(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
        ],
    );

    let dataset_name_1 = "dataset_1";
    service.create_dataset_entry_with_name(dataset_name_1).await;
    service
        .register_with_dataset_name_blocking(dataset_name_1, data_sources_def_1.to_data_sources())
        .await;

    //
    // Create a second dataset
    //

    let data_sources_def_2 = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::nasty("my_segment_id1", &["my/entity", "my/other/entity"]),
            LayerDefinition::nasty("my_segment_id2", &["my/other/entity"]),
        ],
    );

    let dataset_name_2 = "dataset_2";
    service.create_dataset_entry_with_name(dataset_name_2).await;
    service
        .register_with_dataset_name_blocking(dataset_name_2, data_sources_def_2.to_data_sources())
        .await;

    //
    // Query some chunks from dataset 1
    //

    let mut chunk_info_1 = service
        .query_dataset(
            tonic::Request::new(
                QueryDatasetRequest {
                    scan_parameters: Some(ScanParameters {
                        // TODO(RR-2677): when `required_column_names` contains only the chunk key,
                        // the chunk id will have to be added here
                        columns: FetchChunksRequest::required_column_names(),
                        ..Default::default()
                    }),
                    entity_paths: vec!["my/entity".into()],
                    select_all_entity_paths: false,
                    ..Default::default()
                }
                .into(),
            )
            .with_entry_name(dataset_name_1)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().data))
        .map(|dfp| dfp.try_into().unwrap())
        .collect::<Vec<_>>()
        .await;

    //
    // Query some chunks from dataset 2
    //

    let chunk_info_2 = service
        .query_dataset(
            tonic::Request::new(
                QueryDatasetRequest {
                    scan_parameters: Some(ScanParameters {
                        columns: FetchChunksRequest::required_column_names(),
                        ..Default::default()
                    }),
                    entity_paths: vec!["my/other/entity".into()],
                    select_all_entity_paths: false,
                    ..Default::default()
                }
                .into(),
            )
            .with_entry_name(dataset_name_1)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().data))
        .map(|dfp| dfp.try_into().unwrap())
        .collect::<Vec<_>>()
        .await;

    //
    // Request all chunks.
    //

    chunk_info_1.extend(chunk_info_2);
    let chunk_info = concat_record_batches(&chunk_info_1);

    let chunks = service
        .fetch_chunks(tonic::Request::new(FetchChunksRequest {
            chunk_infos: vec![chunk_info.clone().into()],
        }))
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().chunks))
        .map(|msg| re_chunk::Chunk::from_arrow_msg(&msg.to_application(()).unwrap()))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    //
    // Check we have everything.
    //

    let requested_ids = Tuid::from_arrow(
        chunk_info
            .column_by_name(QueryDatasetResponse::FIELD_CHUNK_ID)
            .unwrap(),
    )
    .unwrap()
    .into_iter()
    .collect::<HashSet<_>>();

    let received_ids = chunks
        .into_iter()
        .map(|chunk| chunk.id().as_tuid())
        .collect::<HashSet<_>>();

    assert_eq!(requested_ids, received_ids);
}
