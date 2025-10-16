use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, GetDatasetSchemaRequest,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use super::common::{
    DataSourcesDefinition, LayerDefinition, LayerType, register_with_dataset_name,
};
use crate::SchemaExt as _;

pub async fn simple_dataset_schema(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new([
        LayerDefinition {
            partition_id: "my_partition_id1",
            layer_name: None,
            layer_type: LayerType::simple(&["my/entity", "my/other/entity"]),
        },
        LayerDefinition {
            partition_id: "my_partition_id2",
            layer_name: None,
            layer_type: LayerType::simple(&["my/entity"]),
        },
        LayerDefinition {
            partition_id: "my_partition_id3",
            layer_name: None,
            layer_type: LayerType::simple(&["my/entity", "another/one", "yet/another/one"]),
        },
    ]);

    let dataset_name = "my_dataset1";
    service
        .create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
            name: Some(dataset_name.to_owned()),
            id: None,
        }))
        .await
        .unwrap();

    register_with_dataset_name(&service, dataset_name, data_sources_def.to_data_sources()).await;

    dataset_schema_snapshot(&service, dataset_name, "simple_dataset").await;
}

pub async fn empty_dataset_schema(service: impl RerunCloudService) {
    let dataset_name = "empty_dataset";
    service
        .create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
            name: Some(dataset_name.to_owned()),
            id: None,
        }))
        .await
        .unwrap();

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
