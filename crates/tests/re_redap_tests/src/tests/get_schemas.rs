//! End-to-end coverage for `GetSchemas`, the lookup of layer schemas by the hashes that
//! `ScanDatasetManifest` reports.

use std::collections::BTreeSet;

use arrow::array::{FixedSizeBinaryArray, RecordBatch};
use arrow::datatypes::Schema;
use futures::TryStreamExt as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_index::RawRrdManifest;
use re_protos::cloud::v1alpha1::ext::{self, ScanDatasetManifestDataframe};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{GetSchemasRequest, ScanDatasetManifestRequest};
use re_protos::common::v1alpha1::ext::SchemaId;
use re_protos::headers::RerunHeadersInjectorExt as _;

use super::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};

const DATASET: &str = "get_schemas_dataset";
const OTHER_DATASET: &str = "get_schemas_other_dataset";

pub async fn get_schemas(service: impl RerunCloudService) {
    // `run-1` and `run-2` share a schema on the base layer; the `extra` layer has another one.
    register(
        &service,
        DATASET,
        1,
        [
            LayerDefinition::simple("run-1", &["my/entity"]),
            LayerDefinition::simple("run-2", &["my/entity"]),
            LayerDefinition::simple("run-1", &["other/entity"]).layer_name("extra"),
        ],
    )
    .await;

    let manifest_ids = manifest_schema_ids(&service, DATASET).await;
    assert_eq!(manifest_ids.len(), 2, "expected two distinct layer schemas");

    // Duplicates are sent on the wire on purpose: the clean request type would drop them.
    let unknown_id = SchemaId::from_sha256([0xab; 32]);
    let mut requested: Vec<SchemaId> = manifest_ids.iter().copied().collect();
    requested.push(unknown_id);
    requested.push(requested[0]);

    let schemas = get_schemas_ok(&service, DATASET, &requested).await;

    let returned_ids: Vec<SchemaId> = schemas.iter().map(|response| response.schema_id).collect();
    assert_eq!(
        returned_ids.len(),
        manifest_ids.len(),
        "each known identifier must be returned exactly once, and unknown ones not at all"
    );
    assert_eq!(
        returned_ids.iter().copied().collect::<BTreeSet<_>>(),
        manifest_ids
    );

    let mut entity_sets = BTreeSet::new();
    for ext::GetSchemasResponse { schema_id, schema } in &schemas {
        assert_eq!(
            &RawRrdManifest::compute_sorbet_schema_sha256(schema).unwrap(),
            schema_id.sha256(),
            "the returned schema must hash to the identifier it was looked up by"
        );
        entity_sets.insert(component_entities(schema));
    }
    assert_eq!(
        entity_sets,
        BTreeSet::from([
            BTreeSet::from(["/my/entity".to_owned()]),
            BTreeSet::from(["/other/entity".to_owned()]),
        ]),
        "schemas must be returned unmerged"
    );
}

pub async fn get_schemas_empty_request(service: impl RerunCloudService) {
    register(
        &service,
        DATASET,
        1,
        [LayerDefinition::simple("run-1", &["my/entity"])],
    )
    .await;

    assert!(get_schemas_ok(&service, DATASET, &[]).await.is_empty());
}

pub async fn get_schemas_other_dataset_id(service: impl RerunCloudService) {
    register(
        &service,
        DATASET,
        1,
        [LayerDefinition::simple("run-1", &["my/entity"])],
    )
    .await;
    register(
        &service,
        OTHER_DATASET,
        2,
        [LayerDefinition::simple("run-1", &["other/entity"])],
    )
    .await;

    let other_ids = manifest_schema_ids(&service, OTHER_DATASET).await;
    assert_eq!(other_ids.len(), 1);
    assert!(
        manifest_schema_ids(&service, DATASET)
            .await
            .is_disjoint(&other_ids)
    );

    let requested: Vec<SchemaId> = other_ids.into_iter().collect();
    assert!(
        get_schemas_ok(&service, DATASET, &requested)
            .await
            .is_empty(),
        "an identifier that only another dataset holds must not be returned"
    );
}

pub async fn get_schemas_invalid_id(service: impl RerunCloudService) {
    register(
        &service,
        DATASET,
        1,
        [LayerDefinition::simple("run-1", &["my/entity"])],
    )
    .await;

    let invalid_ids = [
        re_protos::common::v1alpha1::SchemaId {
            sha256: Some(vec![1, 2, 3].into()),
        },
        re_protos::common::v1alpha1::SchemaId { sha256: None },
    ];
    for invalid_id in invalid_ids {
        let status = service
            .get_schemas(
                tonic::Request::new(GetSchemasRequest {
                    schema_ids: vec![invalid_id.clone()],
                })
                .with_entry_name(entry_name(DATASET)),
            )
            .await
            .err()
            .unwrap_or_else(|| panic!("{invalid_id:?} must be rejected"));
        assert_eq!(status.code(), tonic::Code::InvalidArgument, "{status:?}");
    }
}

pub async fn get_schemas_too_many_ids(service: impl RerunCloudService) {
    register(
        &service,
        DATASET,
        1,
        [LayerDefinition::simple("run-1", &["my/entity"])],
    )
    .await;

    let manifest_ids = manifest_schema_ids(&service, DATASET).await;
    let known_id = *manifest_ids.first().unwrap();

    // The limit counts duplicates, so a request at the limit still returns the one schema.
    let at_limit = vec![known_id; ext::GetSchemasRequest::MAX_SCHEMA_IDS];
    assert_eq!(get_schemas_ok(&service, DATASET, &at_limit).await.len(), 1);

    let over_limit = vec![known_id; ext::GetSchemasRequest::MAX_SCHEMA_IDS + 1];
    let status = service
        .get_schemas(
            tonic::Request::new(GetSchemasRequest {
                schema_ids: over_limit.into_iter().map(Into::into).collect(),
            })
            .with_entry_name(entry_name(DATASET)),
        )
        .await
        .err()
        .expect("a request over the limit must be rejected");
    assert_eq!(status.code(), tonic::Code::InvalidArgument, "{status:?}");
}

// ---

async fn register(
    service: &impl RerunCloudService,
    dataset_name: &str,
    tuid_prefix: u64,
    layers: impl IntoIterator<Item = LayerDefinition>,
) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(tuid_prefix, layers);
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
}

/// The distinct non-null layer schema identifiers of the dataset manifest.
async fn manifest_schema_ids(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> BTreeSet<SchemaId> {
    let column_name = ScanDatasetManifestDataframe::COLUMN_RERUN_SCHEMA_SHA256.name;
    let responses: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest {
                columns: vec![column_name.to_owned()],
                ..Default::default()
            })
            .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    let mut ids = BTreeSet::new();
    for response in responses {
        let batch: RecordBatch = response.data.unwrap().try_into().unwrap();
        let column = batch
            .column_by_name(column_name)
            .unwrap()
            .downcast_array_ref::<FixedSizeBinaryArray>()
            .unwrap();
        ids.extend(
            column
                .iter()
                .flatten()
                .map(|sha256| SchemaId::from_sha256(sha256.try_into().unwrap())),
        );
    }
    ids
}

async fn get_schemas_ok(
    service: &impl RerunCloudService,
    dataset_name: &str,
    schema_ids: &[SchemaId],
) -> Vec<ext::GetSchemasResponse> {
    let responses: Vec<_> = service
        .get_schemas(
            tonic::Request::new(GetSchemasRequest {
                schema_ids: schema_ids.iter().copied().map(Into::into).collect(),
            })
            .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    responses
        .into_iter()
        .map(|response| response.try_into().unwrap())
        .collect()
}

/// The entity paths of the schema's component columns, excluding properties.
fn component_entities(schema: &Schema) -> BTreeSet<String> {
    re_sorbet::SorbetColumnDescriptors::try_from_arrow_fields(None, schema.fields())
        .unwrap()
        .component_columns()
        .filter(|column| !column.entity_path.is_property())
        .map(|column| column.entity_path.to_string())
        .collect()
}
