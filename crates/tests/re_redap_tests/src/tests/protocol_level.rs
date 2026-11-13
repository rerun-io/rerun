use futures::TryStreamExt as _;
use re_protos::cloud::v1alpha1::ext::{
    ChunkMode, ContentSelection, DirectUrlMode, FindChunksRequest, QueryDatasetRequest,
    SegmentSelection,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{self as cloud_proto, DatasetQueryScope, GetRrdManifestRequest};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::protocol_level::ProtocolLevel;

use super::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};

/// A dataset requires no protocol level until the server raises it, so a client from before
/// protocol levels existed can read it through every RPC that hands out chunks.
pub async fn new_dataset_serves_clients_without_protocol_level(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    let segment_name = "my_segment";
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple(segment_name, &["my/entity"])],
    );

    let entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    assert_eq!(
        entry.required_client_protocol_level,
        ProtocolLevel::BASELINE
    );

    let manifest_parts: Vec<_> = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some(segment_name.into()),
                generate_direct_urls: false,
                client_meta: None,
            })
            .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap_or_else(|err| panic!("get_rrd_manifest must succeed: {err}"))
        .into_inner()
        .try_collect()
        .await
        .unwrap_or_else(|err| panic!("get_rrd_manifest stream must not error: {err}"));
    assert!(!manifest_parts.is_empty());

    let find_chunks_request = cloud_proto::FindChunksRequest {
        client_meta: None,
        ..FindChunksRequest {
            contents: ContentSelection::All,
            mode: ChunkMode::Static {
                segments: SegmentSelection::All,
            },
            direct_url_mode: DirectUrlMode::Signed,
            scope: DatasetQueryScope::Full,
        }
        .into()
    };
    let candidates: Vec<_> = service
        .find_chunks(
            tonic::Request::new(find_chunks_request).with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap_or_else(|err| panic!("find_chunks must succeed: {err}"))
        .into_inner()
        .try_collect()
        .await
        .unwrap_or_else(|err| panic!("find_chunks stream must not error: {err}"));
    assert!(!candidates.is_empty());

    let query_dataset_request = cloud_proto::QueryDatasetRequest {
        client_meta: None,
        ..QueryDatasetRequest::default().into()
    };
    let chunks: Vec<_> = service
        .query_dataset(
            tonic::Request::new(query_dataset_request).with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap_or_else(|err| panic!("query_dataset must succeed: {err}"))
        .into_inner()
        .try_collect()
        .await
        .unwrap_or_else(|err| panic!("query_dataset stream must not error: {err}"));
    assert!(!chunks.is_empty());
}
