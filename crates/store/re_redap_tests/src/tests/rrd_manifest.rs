use itertools::Itertools as _;

use re_arrow_util::{RecordBatchTestExt as _, SchemaTestExt as _};
use re_protos::cloud::v1alpha1::FetchChunksRequest;
use re_protos::cloud::v1alpha1::GetRrdManifestRequest;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_sdk::external::re_log_encoding::RrdManifest;
use re_sdk::external::re_log_encoding::ToApplication as _;

use super::common::{DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _};

pub async fn simple_dataset_rrd_manifest(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("my_segment", &["my/entity"]), //
        ],
    );

    let dataset_name = "my_dataset";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let segment_id = SegmentId::new("my_segment".to_owned());
    let rrd_manifest_batch_result =
        dataset_rrd_manifest_snapshot(&service, segment_id, dataset_name).await;

    let rrd_manifest = match rrd_manifest_batch_result {
        Ok(rrd_manifest) => rrd_manifest,
        Err(status) => {
            if status.code() == tonic::Code::Unimplemented {
                return; // TODO(RR-3110): implemented this endpoint on Rerun Cloud
            } else {
                panic!("tonic error: {status}");
            }
        }
    };

    use futures::StreamExt as _;
    let mut chunks = service
        .fetch_chunks(tonic::Request::new(FetchChunksRequest {
            chunk_infos: vec![rrd_manifest.data.clone().into()],
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

    insta::assert_snapshot!("fetch_chunks_from_rrd_manifest", printed);
}

// ---

async fn dataset_rrd_manifest_snapshot(
    service: &impl RerunCloudService,
    segment_id: SegmentId,
    dataset_name: &str,
) -> tonic::Result<RrdManifest> {
    let rrd_manifest = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some(segment_id.into()),
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await?
        .into_inner()
        .rrd_manifest
        .unwrap()
        .to_application(())
        .unwrap();

    insta::assert_snapshot!("rrd_manifest", rrd_manifest.data.format_snapshot(true));
    insta::assert_snapshot!(
        "rrd_manifest_sorbet_schema",
        rrd_manifest.sorbet_schema.format_snapshot(),
    );
    insta::assert_snapshot!(
        "rrd_manifest_sorbet_schema_sha256",
        rrd_manifest
            .sorbet_schema_sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    );

    Ok(rrd_manifest)
}
