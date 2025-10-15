use futures::StreamExt as _;
use itertools::Itertools as _;
use url::Url;

use re_log_encoding::codec::wire::{decoder::Decode as _, encoder::Encode as _};
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, FetchChunksRequest, QueryDatasetResponse,
        ext::QueryDatasetRequest, rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::tests::common;
use crate::{RecordBatchExt as _, create_simple_recording};

/// This test makes a snapshot of all the chunks returned for a simple dataset.
///
/// In general, there is no guarantee made on the chunk representation of the underlying data (aka
/// chunks can be split/compacted/etc. arbitrarily by the implementation). So this test is
/// conceptually incorrect and works only because the data/chunk layout used is very basic and
/// predictable.
pub async fn simple_dataset_fetch_chunk_snapshot(fe: impl RerunCloudService) {
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

    let chunk_keys = common::concat_record_batches(&chunk_info)
        .sort_rows_by(&[QueryDatasetResponse::FIELD_CHUNK_ID])
        .unwrap()
        .filtered_columns(&[
            QueryDatasetResponse::FIELD_CHUNK_KEY,
            //TODO(RR-2677): remove when these columns are no longer required
            QueryDatasetResponse::FIELD_CHUNK_ID,
            QueryDatasetResponse::FIELD_CHUNK_PARTITION_ID,
            QueryDatasetResponse::FIELD_CHUNK_LAYER_NAME,
        ]);

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
