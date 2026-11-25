use futures::TryStreamExt as _;
use re_protos::cloud::v1alpha1::ext::{
    ChunkMode, ContentSelection, DirectUrlMode, FindChunksRequest, QueryDatasetRequest,
    SegmentSelection,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    self as cloud_proto, ClientMeta, DatasetQueryScope, GetRrdManifestRequest,
};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::protocol_level::ProtocolLevel;

use super::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};

/// A server advertises its protocol level on `/WhoAmI`. Both servers implement the server part of
/// every level up to [`ProtocolLevel::CHUNK_SEGMENT_FROM_REQUEST`], and a server that advertises
/// nothing reads as [`ProtocolLevel::BASELINE`].
pub async fn who_am_i_advertises_protocol_level(service: impl RerunCloudService) {
    let response = service
        .who_am_i(tonic::Request::new(cloud_proto::WhoAmIRequest {}))
        .await
        .expect("WhoAmI must succeed")
        .into_inner();

    let level = ProtocolLevel::from(response.protocol_level);
    assert!(
        level >= ProtocolLevel::CHUNK_SEGMENT_FROM_REQUEST,
        "the server advertises protocol level {level}"
    );
}

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

    for rpc in ChunkRpc::ALL {
        rpc.call(&service, dataset_name, segment_name, None)
            .await
            .unwrap_or_else(|err| panic!("{rpc:?} must succeed: {err}"));
    }
}

/// A segment id override makes a dataset require [`ProtocolLevel::CHUNK_SEGMENT_FROM_REQUEST`],
/// so every RPC that hands out chunks refuses clients below it, and still serves the others.
pub async fn dataset_with_segment_id_override_refuses_older_clients(
    service: impl RerunCloudService,
) {
    let dataset_name = "dataset_refuses_older_clients";
    let segment_name = "my_segment";
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        730,
        [LayerDefinition::simple("my_recording", &["my/entity"])],
    );
    let data_sources = data_sources_def
        .to_data_sources_ext()
        .into_iter()
        .map(|data_source| {
            data_source
                .with_segment_id_override(segment_name.into())
                .into()
        })
        .collect();

    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources)
        .await;

    let older_clients = [
        None,
        Some(ClientMeta {
            protocol_level: ProtocolLevel::MANIFEST_KEY_STORE_ID.as_u32(),
        }),
    ];
    for rpc in ChunkRpc::ALL {
        for client_meta in older_clients {
            let status = rpc
                .call(&service, dataset_name, segment_name, client_meta)
                .await
                .expect_err("a client below the required level must be refused");
            assert_eq!(
                status.code(),
                tonic::Code::FailedPrecondition,
                "{rpc:?} with {client_meta:?}: {status:?}"
            );
        }

        rpc.call(
            &service,
            dataset_name,
            segment_name,
            Some(ClientMeta::current()),
        )
        .await
        .unwrap_or_else(|err| panic!("{rpc:?} must serve a current client: {err}"));
    }
}

/// The RPCs that hand out chunks, and so check the client's protocol level.
#[derive(Debug, Clone, Copy)]
enum ChunkRpc {
    GetRrdManifest,
    FindChunks,
    QueryDataset,
}

impl ChunkRpc {
    const ALL: [Self; 3] = [Self::GetRrdManifest, Self::FindChunks, Self::QueryDataset];

    /// Calls the RPC as a client sending `client_meta`, and checks that it returns something.
    async fn call(
        self,
        service: &impl RerunCloudService,
        dataset_name: &str,
        segment_name: &str,
        client_meta: Option<ClientMeta>,
    ) -> tonic::Result<()> {
        let returned_something = match self {
            Self::GetRrdManifest => {
                let request = GetRrdManifestRequest {
                    segment_id: Some(segment_name.into()),
                    generate_direct_urls: false,
                    client_meta,
                };
                let parts: Vec<_> = service
                    .get_rrd_manifest(
                        tonic::Request::new(request).with_entry_name(entry_name(dataset_name)),
                    )
                    .await?
                    .into_inner()
                    .try_collect()
                    .await?;
                !parts.is_empty()
            }

            Self::FindChunks => {
                let request = cloud_proto::FindChunksRequest {
                    client_meta,
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
                        tonic::Request::new(request).with_entry_name(entry_name(dataset_name)),
                    )
                    .await?
                    .into_inner()
                    .try_collect()
                    .await?;
                !candidates.is_empty()
            }

            Self::QueryDataset => {
                let request = cloud_proto::QueryDatasetRequest {
                    client_meta,
                    ..QueryDatasetRequest::default().into()
                };
                let chunks: Vec<_> = service
                    .query_dataset(
                        tonic::Request::new(request).with_entry_name(entry_name(dataset_name)),
                    )
                    .await?
                    .into_inner()
                    .try_collect()
                    .await?;
                !chunks.is_empty()
            }
        };

        assert!(returned_something, "{self:?} must return something");
        Ok(())
    }
}
