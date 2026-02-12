use std::sync::Arc;

use arrow::array::{Float32Array, RecordBatch};
use itertools::Itertools as _;
use re_arrow_util::{RecordBatchExt as _, RecordBatchTestExt as _, SchemaTestExt as _};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    FetchChunksRequest, GetRrdManifestRequest, ScanSegmentTableRequest,
};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_sdk::AsComponents;
use re_sdk::external::re_log_encoding::{RawRrdManifest, ToApplication as _};
use re_sdk_types::AnyValues;

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
    let rrd_manifest =
        dataset_rrd_manifest_snapshot(&service, segment_id, dataset_name, "rrd_manifest")
            .await
            .unwrap();

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

pub async fn layered_segment(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    let segment_name = "my_segment";

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple(segment_name, &["my/entity"]).layer_name("base"), //
            LayerDefinition::static_components(
                segment_name,
                [
                    (
                        "/data".into(),
                        Box::new(AnyValues::default().with_component_from_data(
                            "test",
                            Arc::new(Float32Array::from(vec![1.0f32, 2.0, 3.0])),
                        )) as Box<dyn AsComponents>,
                    ), //
                ],
            )
            .layer_name("extra"),
        ],
    );

    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    dataset_rrd_manifest_snapshot(
        &service,
        segment_name.into(),
        dataset_name,
        "layered_segment_rrd_manifest_1_all_there",
    )
    .await
    .unwrap();

    service
        .unregister_from_dataset_name(dataset_name, &[], &["base"])
        .await
        .unwrap();

    dataset_rrd_manifest_snapshot(
        &service,
        segment_name.into(),
        dataset_name,
        "layered_segment_rrd_manifest_2_base_removed",
    )
    .await
    .unwrap();

    service
        .unregister_from_dataset_name(dataset_name, &[], &["extra"])
        .await
        .unwrap();

    let res = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some(segment_name.into()),
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await;
    assert_eq!(tonic::Code::NotFound, res.err().unwrap().code());
}

pub async fn layered_segment_stress(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    let segment_name = "my_segment";

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::nasty(segment_name, &["my/entity/1"]).layer_name("nasty1"),
            LayerDefinition::nasty(segment_name, &["my/entity/2"]).layer_name("nasty2"),
            LayerDefinition::simple(segment_name, &["my/entity/2"]).layer_name("base1"), //
            LayerDefinition::simple(segment_name, &["my/entity/3"]).layer_name("base2"), //
            LayerDefinition::static_components(
                segment_name,
                [
                    (
                        "/my/entity/3".into(),
                        Box::new(AnyValues::default().with_component_from_data(
                            "test",
                            Arc::new(Float32Array::from(vec![1.0f32, 2.0, 3.0])),
                        )) as Box<dyn AsComponents>,
                    ), //
                ],
            )
            .layer_name("extra1"),
            LayerDefinition::static_components(
                segment_name,
                [
                    (
                        "/my/entity/4".into(),
                        Box::new(AnyValues::default().with_component_from_data(
                            "test",
                            Arc::new(Float32Array::from(vec![1.0f32, 2.0, 3.0])),
                        )) as Box<dyn AsComponents>,
                    ), //
                ],
            )
            .layer_name("extra2"),
        ],
    );

    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let _res = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some(segment_name.into()),
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await
        .unwrap();

    for i in 1..=2 {
        for layer in ["base", "nasty", "extra"] {
            let layer = format!("{layer}{i}");
            service
                .unregister_from_dataset_name(dataset_name, &[], &[&layer])
                .await
                .unwrap();

            let all_removed = {
                use futures::TryStreamExt as _;

                let responses: Vec<_> = service
                    .scan_segment_table(
                        tonic::Request::new(ScanSegmentTableRequest { columns: vec![] })
                            .with_entry_name(dataset_name)
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .into_inner()
                    .try_collect()
                    .await
                    .unwrap();

                let batches: Vec<RecordBatch> = responses
                    .into_iter()
                    .map(|resp| resp.data.unwrap().try_into().unwrap())
                    .collect_vec();

                let batch = arrow::compute::concat_batches(
                    batches
                        .first()
                        .expect("there should be at least one batch")
                        .schema_ref(),
                    &batches,
                )
                .unwrap();

                batch.num_rows() == 0
            };

            let res = service
                .get_rrd_manifest(
                    tonic::Request::new(GetRrdManifestRequest {
                        segment_id: Some(segment_name.into()),
                    })
                    .with_entry_name(dataset_name)
                    .unwrap(),
                )
                .await;

            if all_removed {
                assert_eq!(tonic::Code::NotFound, res.err().unwrap().code());
            } else {
                assert!(res.is_ok());
            }
        }
    }
}

pub async fn unregistered_segment(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("my_segment_id", &["my/entity", "my/other/entity"]), //
        ],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    service
        .unregister_from_dataset_name(dataset_name, &["my_segment_id"], &[])
        .await
        .unwrap();

    let res = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some("my_segment_id".into()),
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await;
    assert_eq!(tonic::Code::NotFound, res.err().unwrap().code());
}

pub async fn segment_id_not_found(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let segment_id = SegmentId::new("my_segment".to_owned());
    let res = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some(segment_id.into()),
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await;

    assert_eq!(tonic::Code::NotFound, res.err().unwrap().code());
}

// ---

async fn dataset_rrd_manifest_snapshot(
    service: &impl RerunCloudService,
    segment_id: SegmentId,
    dataset_name: &str,
    snapshot_name: &str,
) -> tonic::Result<RawRrdManifest> {
    let responses = service
        .get_rrd_manifest(
            tonic::Request::new(GetRrdManifestRequest {
                segment_id: Some(segment_id.into()),
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await?
        .into_inner();

    let mut rrd_manifest: Option<RawRrdManifest> = None;

    use futures::{StreamExt as _, pin_mut};
    pin_mut!(responses);
    while let Some(resp) = responses.next().await {
        let rrd_manifest_part = resp
            .unwrap()
            .rrd_manifest
            .unwrap()
            .to_application(())
            .unwrap();

        if let Some(mut temp) = rrd_manifest.take() {
            temp.data =
                re_arrow_util::concat_polymorphic_batches(&[temp.data, rrd_manifest_part.data])
                    .unwrap();
            rrd_manifest = Some(temp);
        } else {
            rrd_manifest = Some(rrd_manifest_part);
        }
    }

    let rrd_manifest = rrd_manifest.unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}"),
        rrd_manifest
            .data
            // Chunk offsets and sizes cannot possibly align across different implementations that
            // store data differently.
            // The actual values don't matter in any case, as long as we're able to use the
            // returned data to fetch the associated chunks, which we check above.
            .redact(&[
                RawRrdManifest::FIELD_CHUNK_KEY,
                RawRrdManifest::FIELD_CHUNK_BYTE_OFFSET,
                RawRrdManifest::FIELD_CHUNK_BYTE_SIZE,
                RawRrdManifest::FIELD_CHUNK_BYTE_SIZE_UNCOMPRESSED,
            ])
            // Implementation-specific fields shouldn't be compared at all.
            .filter_columns_by(
                |f| !RawRrdManifest::COMMON_IMPL_SPECIFIC_FIELDS.contains(&f.name().as_str())
            )
            .unwrap()
            .horizontally_sorted()
            .format_snapshot(true)
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_sorbet_schema"),
        rrd_manifest.sorbet_schema.format_snapshot(),
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_sorbet_schema_sha256"),
        rrd_manifest
            .sorbet_schema_sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    );

    Ok(rrd_manifest)
}
