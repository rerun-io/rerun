use futures::StreamExt as _;
use itertools::Itertools as _;

use re_log_encoding::codec::wire::{decoder::Decode as _, encoder::Encode as _};
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, FetchChunksRequest, QueryDatasetResponse,
        ext::QueryDatasetRequest, rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::RecordBatchExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, concat_record_batches, register_with_dataset_name,
};

/// This test makes a snapshot of all the chunks returned for a simple dataset.
///
/// In general, there is no guarantee made on the chunk representation of the underlying data (aka
/// chunks can be split/compacted/etc. arbitrarily by the implementation). So this test is
/// conceptually incorrect and works only because the data/chunk layout used is very basic and
/// predictable.
pub async fn simple_dataset_fetch_chunk_snapshot(fe: impl RerunCloudService) {
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

    let chunk_info = fe
        .query_dataset(
            tonic::Request::new(QueryDatasetRequest::default().into())
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

    let required_columns = FetchChunksRequest::required_column_names();
    let required_columns_ref = required_columns.iter().map(|s| s.as_str()).collect_vec();
    let chunk_keys = concat_record_batches(&chunk_info)
        .sort_rows_by(&[QueryDatasetResponse::FIELD_CHUNK_ID])
        .unwrap()
        .filtered_columns(&required_columns_ref);

    let mut chunks = fe
        .fetch_chunks(tonic::Request::new(FetchChunksRequest {
            chunk_infos: vec![chunk_keys.encode().unwrap()],
        }))
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().chunks))
        .map(|msg| {
            re_chunk::Chunk::from_arrow_msg(
                &re_log_encoding::protobuf_conversions::arrow_msg_from_proto(&msg).unwrap(),
            )
        })
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
