#![expect(clippy::unwrap_used)]

use super::common::{DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _};
use crate::tests::common::concat_record_batches;
use crate::{FieldsTestExt as _, RecordBatchTestExt as _, SchemaTestExt as _};
use arrow::array::{RecordBatch, StringArray};
use arrow::datatypes::Schema;
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_arrow_util::{ArrowArrayDowncastRef as _, RecordBatchExt as _};
use re_protos::cloud::v1alpha1::ext::{LayerRegistrationStatus, QueryDatasetRequest};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    GetDatasetManifestSchemaRequest, GetSegmentTableSchemaRequest, QueryDatasetResponse,
    ReadDatasetEntryRequest, ScanDatasetManifestRequest, ScanDatasetManifestResponse,
    ScanSegmentTableRequest, ScanSegmentTableResponse,
};
use re_protos::headers::RerunHeadersInjectorExt as _;

pub async fn unregister_simple(service: impl RerunCloudService) {
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

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    let dataset_updated_at_1 = get_dataset_updated_at_nanos(&service, dataset_name).await;
    {
        let snapshot_name = "simple_1_register_all";
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }

    let removed = service
        .unregister_from_dataset_name(dataset_name, &["my_segment_id2"], &["base"])
        .await;
    let dataset_updated_at_2 = get_dataset_updated_at_nanos(&service, dataset_name).await;
    {
        let snapshot_name = "simple_2_remove_segment_id2";
        snapshot_response(
            &service,
            dataset_name,
            snapshot_name,
            removed.expect("removal should succeed"),
        )
        .await;
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }

    let removed = service
        .unregister_from_dataset_name(
            dataset_name,
            &["my_segment_id1", "my_segment_id3"],
            &["base"],
        )
        .await;
    let dataset_updated_at_3 = get_dataset_updated_at_nanos(&service, dataset_name).await;
    {
        let snapshot_name = "simple_3_remove_remaining_segments";
        snapshot_response(
            &service,
            dataset_name,
            snapshot_name,
            removed.expect("removal should succeed"),
        )
        .await;
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }

    // Make sure re-registering on top of an unregistered segment doesn't do anything weird.
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    let dataset_updated_at_4 = get_dataset_updated_at_nanos(&service, dataset_name).await;
    {
        let snapshot_name = "simple_4_reregister_all";
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }

    assert!(
        dataset_updated_at_1 < dataset_updated_at_2,
        "Timestamp should be updated after adding or removing a layer."
    );
    assert!(
        dataset_updated_at_2 < dataset_updated_at_3,
        "Timestamp should be updated after adding or removing a layer."
    );
    assert!(
        dataset_updated_at_3 < dataset_updated_at_4,
        "Timestamp should be updated after adding or removing a layer."
    );
}

pub async fn unregister_products(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("my_segment_id1", &["my/entity"]).layer_name("A"), //
            LayerDefinition::simple("my_segment_id1", &["my/entity"]).layer_name("B"),
            LayerDefinition::simple("my_segment_id1", &["my/entity"]).layer_name("C"),
            LayerDefinition::simple("my_segment_id1", &["my/entity"]).layer_name("D"),
            //
            LayerDefinition::simple("my_segment_id2", &["my/entity"]).layer_name("A"), //
            LayerDefinition::simple("my_segment_id2", &["my/entity"]).layer_name("B"),
            LayerDefinition::simple("my_segment_id2", &["my/entity"]).layer_name("C"),
            LayerDefinition::simple("my_segment_id2", &["my/entity"]).layer_name("D"),
            //
            LayerDefinition::simple("my_segment_id3", &["my/entity"]).layer_name("A"), //
            LayerDefinition::simple("my_segment_id3", &["my/entity"]).layer_name("B"),
            LayerDefinition::simple("my_segment_id3", &["my/entity"]).layer_name("C"),
            LayerDefinition::simple("my_segment_id3", &["my/entity"]).layer_name("D"),
        ],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    {
        let snapshot_name = "products_1_register_all";
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }

    let removed = service
        .unregister_from_dataset_name(
            dataset_name,
            &["my_segment_id1", "my_segment_id3"],
            &["B", "D"],
        )
        .await;
    {
        let snapshot_name = "products_2_remove_layers_BD_for_segments_13";
        snapshot_response(
            &service,
            dataset_name,
            snapshot_name,
            removed.expect("removal should succeed"),
        )
        .await;
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }

    let removed = service
        .unregister_from_dataset_name(dataset_name, &[], &["B", "D"])
        .await;
    {
        let snapshot_name = "products_3_remove_layers_BD_for_all_segments";
        snapshot_response(
            &service,
            dataset_name,
            snapshot_name,
            removed.expect("removal should succeed"),
        )
        .await;
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }

    let removed = service
        .unregister_from_dataset_name(dataset_name, &["my_segment_id2", "my_segment_id3"], &[])
        .await;
    {
        let snapshot_name = "products_4_remove_all_layers_for_segments_23";
        snapshot_response(
            &service,
            dataset_name,
            snapshot_name,
            removed.expect("removal should succeed"),
        )
        .await;
        scan_segment_table_and_snapshot(&service, dataset_name, snapshot_name).await;
        scan_dataset_manifest_and_snapshot(&service, dataset_name, snapshot_name).await;
    }
}

pub async fn unregister_missing_dataset(service: impl RerunCloudService) {
    let dataset_name = "my_dataset_thats_not_there";

    let err = service
        .unregister_from_dataset_name(dataset_name, &["my_segment"], &[])
        .await
        .unwrap_err();
    assert_eq!(tonic::Code::NotFound, err.code());
}

pub async fn unregister_missing_segment(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple(
            "my_segment_id1",
            &["my/entity", "my/other/entity"],
        )],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let removed = service
        .unregister_from_dataset_name(dataset_name, &["some_segment_thats_not_there"], &[])
        .await;
    {
        let snapshot_name = "missing_1_should_be_empty";
        snapshot_response(
            &service,
            dataset_name,
            snapshot_name,
            removed.expect("removal should succeed"),
        )
        .await;
    }
}

pub async fn unregister_invalid_args(service: impl RerunCloudService) {
    {
        let dataset_name = "my_dataset_thats_not_there";

        let err = service
            .unregister_from_dataset_name(dataset_name, &[], &[])
            .await
            .unwrap_err();
        assert_eq!(tonic::Code::NotFound, err.code());
    }

    {
        let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
            1,
            [LayerDefinition::simple(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
            )],
        );

        let dataset_name = "my_dataset1";
        service.create_dataset_entry_with_name(dataset_name).await;
        service
            .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
            .await;

        let err = service
            .unregister_from_dataset_name(dataset_name, &[], &[])
            .await
            .unwrap_err();
        assert_eq!(tonic::Code::InvalidArgument, err.code());
    }
}

// Make sure we cannot find any data when the `status=deleted`.
pub async fn unregister_then_query(service: impl RerunCloudService) {
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

    query_dataset_snapshot(
        &service,
        QueryDatasetRequest::default(),
        dataset_name,
        "unregister_then_query_1_added",
    )
    .await;

    service
        .unregister_from_dataset_name(dataset_name, &["my_segment_id"], &[])
        .await
        .unwrap();

    query_dataset_snapshot(
        &service,
        QueryDatasetRequest::default(),
        dataset_name,
        "unregister_then_query_2_removed",
    )
    .await;
}

// ---

async fn scan_segment_table_and_snapshot(
    service: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
) -> RecordBatch {
    let responses: Vec<_> = service
        .scan_segment_table(
            tonic::Request::new(ScanSegmentTableRequest {
                columns: vec![], // all of them
            })
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

    // check that the _advertised_ schema is consistent with the actual data.
    let alleged_schema: Schema = service
        .get_segment_table_schema(
            tonic::Request::new(GetSegmentTableSchemaRequest {})
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .schema
        .unwrap()
        .try_into()
        .unwrap();

    // Note: we check fields only because some schema-level sorbet metadata is injected in one of
    // the paths and not the other. Anyway, that's what matters.
    assert_eq!(
        alleged_schema.fields(),
        batch.schema_ref().fields(),
        "The actual schema is not consistent with the schema advertised by \
        `get_segment_table_schema`.\n\nActual:\n{}\n\nAlleged:\n{}\n",
        batch.schema().format_snapshot(),
        alleged_schema.format_snapshot(),
    );

    let required_fields = ScanSegmentTableResponse::fields();
    assert!(
        batch.schema().fields().contains_unordered(&required_fields),
        "the schema should contain all the required fields, but it doesn't",
    );

    let unstable_column_names = vec![
        ScanSegmentTableResponse::FIELD_STORAGE_URLS,
        ScanSegmentTableResponse::FIELD_SIZE_BYTES,
        ScanSegmentTableResponse::FIELD_LAST_UPDATED_AT,
    ];
    let filtered_batch = batch
        .remove_columns(&unstable_column_names)
        .auto_sort_rows()
        .unwrap()
        .sort_property_columns()
        .sort_index_columns();

    insta::assert_snapshot!(
        format!("{snapshot_name}_segments_schema"),
        filter_out_index_ranges(batch.clone()).format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_segments_data"),
        filter_out_index_ranges(filtered_batch.clone()).format_snapshot(false)
    );

    batch
}

async fn scan_dataset_manifest_and_snapshot(
    service: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
) -> RecordBatch {
    let responses: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest {
                columns: vec![], // all of them
            })
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

    // check that the _advertised_ schema is consistent with the actual data.
    let alleged_schema: Schema = service
        .get_dataset_manifest_schema(
            tonic::Request::new(GetDatasetManifestSchemaRequest {})
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .schema
        .unwrap()
        .try_into()
        .unwrap();

    // Note: we check fields only because some schema-level sorbet metadata is injected in one of
    // the paths and not the other. Anyway, that's what matters.
    assert_eq!(
        alleged_schema.fields(),
        batch.schema_ref().fields(),
        "The actual schema is not consistent with the schema advertised by \
        `get_dataset_manifest_schema`.\n\nActual:\n{}\n\nAlleged:\n{}\n",
        batch.schema().format_snapshot(),
        alleged_schema.format_snapshot(),
    );

    let required_fields = ScanDatasetManifestResponse::fields();
    assert!(
        batch.schema().fields().contains_unordered(&required_fields),
        "the schema should contain all the required fields, but it doesn't",
    );

    let unstable_column_names = vec![
        // implementation-dependent
        ScanDatasetManifestResponse::FIELD_STORAGE_URL,
        ScanDatasetManifestResponse::FIELD_SIZE_BYTES,
        // unstable
        ScanDatasetManifestResponse::FIELD_LAST_UPDATED_AT,
        ScanDatasetManifestResponse::FIELD_REGISTRATION_TIME,
    ];
    let filtered_batch = batch
        .remove_columns(&unstable_column_names)
        .auto_sort_rows()
        .unwrap()
        .sort_property_columns();

    // For the comparison to make sense across the OSS and enterprise servers, we need to filter
    // out rows where `status=deleted`, since OSS doesn't keep track of removed segments/layers.
    let filtered_batch = {
        let col_status = filtered_batch
            .column_by_name(ScanDatasetManifestResponse::FIELD_REGISTRATION_STATUS)
            .unwrap();
        let col_status = col_status.downcast_array_ref::<StringArray>().unwrap();

        let mask = col_status
            .iter()
            .map(|s| s != Some(LayerRegistrationStatus::Deleted.as_str()))
            .collect_vec();

        arrow::compute::filter_record_batch(&filtered_batch, &mask.into()).unwrap()
    };

    insta::assert_snapshot!(
        format!("{snapshot_name}_manifest_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_manifest_data"),
        filtered_batch.format_snapshot(false)
    );

    batch
}

async fn snapshot_response(
    service: &impl RerunCloudService,
    dataset_name: &str,
    snapshot_name: &str,
    batch: RecordBatch,
) {
    // check that the _advertised_ schema is consistent with the actual data.
    let alleged_schema: Schema = service
        .get_dataset_manifest_schema(
            tonic::Request::new(GetDatasetManifestSchemaRequest {})
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .schema
        .unwrap()
        .try_into()
        .unwrap();

    // Note: we check fields only because some schema-level sorbet metadata is injected in one of
    // the paths and not the other. Anyway, that's what matters.
    assert_eq!(
        alleged_schema.fields(),
        batch.schema_ref().fields(),
        "The actual schema is not consistent with the schema advertised by \
        `get_dataset_manifest_schema`.\n\nActual:\n{}\n\nAlleged:\n{}\n",
        batch.schema().format_snapshot(),
        alleged_schema.format_snapshot(),
    );

    let required_fields = ScanDatasetManifestResponse::fields();
    assert!(
        batch.schema().fields().contains_unordered(&required_fields),
        "the schema should contain all the required fields, but it doesn't",
    );

    let unstable_column_names = vec![
        // implementation-dependent
        ScanDatasetManifestResponse::FIELD_STORAGE_URL,
        ScanDatasetManifestResponse::FIELD_SIZE_BYTES,
        // unstable
        ScanDatasetManifestResponse::FIELD_LAST_UPDATED_AT,
        ScanDatasetManifestResponse::FIELD_REGISTRATION_TIME,
    ];
    let filtered_batch = batch
        .remove_columns(&unstable_column_names)
        .auto_sort_rows()
        .unwrap()
        .sort_property_columns();

    // For the comparison to make sense across the OSS and enterprise servers, we need to filter
    // out rows where `status=deleted`, since OSS doesn't keep track of removed segments/layers.
    let filtered_batch = {
        let col_status = filtered_batch
            .column_by_name(ScanDatasetManifestResponse::FIELD_REGISTRATION_STATUS)
            .unwrap();
        let col_status = col_status.downcast_array_ref::<StringArray>().unwrap();

        let mask = col_status
            .iter()
            .map(|s| s != Some(LayerRegistrationStatus::Deleted.as_str()))
            .collect_vec();

        arrow::compute::filter_record_batch(&filtered_batch, &mask.into()).unwrap()
    };

    insta::assert_snapshot!(
        format!("{snapshot_name}_response_schema"),
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_response_data"),
        filtered_batch.format_snapshot(false)
    );
}

async fn query_dataset_snapshot(
    service: &impl RerunCloudService,
    query_dataset_request: QueryDatasetRequest,
    dataset_name: &str,
    snapshot_name: &str,
) {
    use futures::StreamExt as _;

    let chunk_info = service
        .query_dataset(
            tonic::Request::new(query_dataset_request.into())
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

    let merged_chunk_info = concat_record_batches(&chunk_info);

    // these are the only columns guaranteed to be returned by `query_dataset`
    let required_field = QueryDatasetResponse::fields();

    assert!(
        merged_chunk_info
            .schema()
            .fields()
            .contains_unordered(&required_field),
        "query dataset must return all guaranteed fields\nExpected: {:#?}\nGot: {:#?}",
        required_field,
        merged_chunk_info.schema().fields(),
    );

    let required_column_names = required_field
        .iter()
        .map(|f| f.name().as_str())
        .collect::<Vec<_>>();
    let required_chunk_info = merged_chunk_info
        .project_columns(required_column_names.iter().copied())
        .unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        required_chunk_info.format_schema_snapshot()
    );

    // these columns are not stable, so we cannot snapshot them
    let filtered_chunk_info = required_chunk_info
        .remove_columns(&[
            QueryDatasetResponse::FIELD_CHUNK_KEY,
            QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH,
        ])
        .auto_sort_rows()
        .unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_chunk_info.format_snapshot(false)
    );
}

/// Remove columns such `frame:start`, `log_time:end`, etc.
///
/// There is a fundamental discrepancy between how OSS and DPF handle deleted segments/layers:
/// * OSS actually deletes things and doesn't keep track of any kind of history.
/// * DPF keeps segments around but marks them with a tombstone.
///
/// On its own this is fine, but this interacts with yet another discrepancy: how OSS vs. DPF
/// compute schemas and such.
/// * OSS computes schemas just-in-time, based on whatever data exists at the moment of the call.
/// * DPF materializes all schemas.
///
/// All of this results in OSS dropping index ranges where the last layer using those indexes has
/// been removed, while they will still be there in DPF, as empty columns.
///
/// For now, we just accept it and move on.
fn filter_out_index_ranges(batch: RecordBatch) -> RecordBatch {
    batch
        .filter_columns_by(|f| f.metadata().get("rerun:index_marker").is_none())
        .unwrap()
}

async fn get_dataset_updated_at_nanos(service: &impl RerunCloudService, dataset_name: &str) -> i64 {
    service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {})
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .dataset
        .unwrap()
        .details
        .as_ref()
        .unwrap()
        .updated_at
        .as_ref()
        .map(|ts| ts.seconds * 1_000_000_000 + ts.nanos as i64)
        .unwrap()
}
