use re_protos::{
    cloud::v1alpha1::{GetDatasetSchemaRequest, rerun_cloud_service_server::RerunCloudService},
    headers::RerunHeadersInjectorExt as _,
};

use super::common::{DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _};
use crate::SchemaExt as _;

pub async fn simple_dataset_schema(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new([
        LayerDefinition::simple("my_partition_id1", &["my/entity", "my/other/entity"]),
        LayerDefinition::simple("my_partition_id2", &["my/entity"]),
        LayerDefinition::simple(
            "my_partition_id3",
            &["my/entity", "another/one", "yet/another/one"],
        ),
    ]);

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name(dataset_name, data_sources_def.to_data_sources())
        .await;

    dataset_schema_snapshot(&service, dataset_name, "simple_dataset").await;
}

pub async fn empty_dataset_schema(service: impl RerunCloudService) {
    let dataset_name = "empty_dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    dataset_schema_snapshot(&service, dataset_name, "empty_dataset").await;
}

// ---

async fn dataset_schema_snapshot(
    service: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
) {
    let schema = service
        .get_dataset_schema(
            tonic::Request::new(GetDatasetSchemaRequest {})
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .schema()
        .unwrap();

    insta::assert_snapshot!(format!("{snapshot_name}_schema"), schema.format_snapshot());
}
