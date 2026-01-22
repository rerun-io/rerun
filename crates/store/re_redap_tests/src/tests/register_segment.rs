#![expect(clippy::unwrap_used)]

use arrow::array::{ListArray, RecordBatch, StringArray, TimestampNanosecondArray};
use arrow::datatypes::Schema;
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_log_types::TimeType;
use re_protos::cloud::v1alpha1::ext::{DatasetDetails, RegisterWithDatasetRequest};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    CreateDatasetEntryRequest, DataSource, DataSourceKind, GetDatasetManifestSchemaRequest,
    GetSegmentTableSchemaRequest, ReadDatasetEntryRequest, ScanDatasetManifestRequest,
    ScanDatasetManifestResponse, ScanSegmentTableRequest, ScanSegmentTableResponse, ext,
};
use re_protos::headers::RerunHeadersInjectorExt as _;
use url::Url;

use super::common::{DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, prop};
use crate::{
    FieldsTestExt as _, RecordBatchTestExt as _, SchemaTestExt as _, create_simple_recording_in,
};

pub async fn register_and_scan_simple_dataset(service: impl RerunCloudService) {
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

    scan_segment_table_and_snapshot(&service, dataset_name, "simple").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "simple").await;
}

/// Make sure that registering to blueprint dataset works as expected.
pub async fn register_and_scan_blueprint_dataset(service: impl RerunCloudService) {
    let blueprint_data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        2,
        [LayerDefinition::simple_blueprint("blueprint_segment_id")],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;

    let dataset_details: DatasetDetails = service
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
        .dataset_details
        .unwrap()
        .try_into()
        .unwrap();

    assert!(dataset_details.blueprint_dataset.is_some());

    // find the dataset name for the blueprint dataset
    let blueprint_dataset_name = service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {})
                .with_entry_id(dataset_details.blueprint_dataset.unwrap())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .dataset
        .unwrap()
        .details
        .unwrap()
        .name
        .unwrap();

    service
        .register_with_dataset_name_blocking(
            &blueprint_dataset_name,
            blueprint_data_sources_def.to_data_sources(),
        )
        .await;

    scan_segment_table_and_snapshot(&service, &blueprint_dataset_name, "simple_blueprint").await;
    scan_dataset_manifest_and_snapshot(&service, &blueprint_dataset_name, "simple_blueprint").await;
}

pub async fn register_and_scan_simple_dataset_with_properties(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("my_segment_id1", &["my/entity", "my/other/entity"]),
            LayerDefinition::simple("my_segment_id2", &["my/entity"]),
            LayerDefinition::simple(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
            LayerDefinition::properties(
                "my_segment_id1",
                [prop(
                    "text_log",
                    re_sdk_types::archetypes::TextLog::new("i'm segment 1"),
                )],
            )
            .layer_name("props"),
            LayerDefinition::properties(
                "my_segment_id2",
                [
                    prop(
                        "text_log",
                        re_sdk_types::archetypes::TextLog::new("i'm segment 2"),
                    ),
                    prop(
                        "points",
                        re_sdk_types::archetypes::Points2D::new([(0.0, 1.0)]),
                    ),
                ],
            )
            .layer_name("props"),
        ],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    scan_segment_table_and_snapshot(&service, dataset_name, "simple_with_properties").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "simple_with_properties").await;
}

/// This test checks that the registration order takes precedence to resolve a segment's
/// properties.
///
/// Note: this is not great. We should probably use the "regular" Rerun way for that (aka row id
/// timestamp). But this is not how Rerun Cloud is currently working, and consistency is better than
/// correctness for the OSS server.
pub async fn register_and_scan_simple_dataset_with_properties_out_of_order(
    service: impl RerunCloudService,
) {
    let first_logged_data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        10, // <- mind this
        [LayerDefinition::properties(
            "my_segment_id1",
            [prop(
                "text_log",
                re_sdk_types::archetypes::TextLog::new(
                    "I was logged first, registered last, so I should win",
                ),
            )],
        )
        .layer_name("prop1")],
    );
    let first_logged_data_sources = first_logged_data_sources_def.to_data_sources();

    let last_logged_data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        20, // <- mind this
        [LayerDefinition::properties(
            "my_segment_id1",
            [prop(
                "text_log",
                re_sdk_types::archetypes::TextLog::new("I was logged last, registered first"),
            )],
        )
        .layer_name("prop2")],
    );
    let last_logged_data_sources = last_logged_data_sources_def.to_data_sources();

    let dataset_name = "my_dataset";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, last_logged_data_sources)
        .await;

    service
        .register_with_dataset_name_blocking(dataset_name, first_logged_data_sources)
        .await;

    let dataset_manifest =
        scan_dataset_manifest_and_snapshot(&service, dataset_name, "out_of_order_properties").await;
    scan_segment_table_and_snapshot(&service, dataset_name, "out_of_order_properties").await;

    // assert test correctness
    let registration_time_col = dataset_manifest
        .column_by_name(ScanDatasetManifestResponse::FIELD_REGISTRATION_TIME)
        .unwrap()
        .downcast_array_ref::<TimestampNanosecondArray>()
        .unwrap();

    let prop_col = dataset_manifest
        .column_by_name("property:text_log:TextLog:text")
        .unwrap()
        .downcast_array_ref::<ListArray>()
        .unwrap();

    assert!(registration_time_col.value(0) < registration_time_col.value(1));

    assert_eq!(
        prop_col
            .value(0)
            .downcast_array_ref::<StringArray>()
            .unwrap()
            .value(0),
        "I was logged last, registered first"
    );
}

pub async fn register_and_scan_simple_dataset_with_layers(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("segment1", &["my/entity", "another/one", "yet/another/one"]),
            LayerDefinition::simple("segment1", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("segment2", &["another/one", "yet/another/one"])
                .layer_name("base"),
            LayerDefinition::simple("segment2", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("segment3", &["i/am/alone"]),
        ],
    );

    let dataset_name = "dataset_with_layers";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    scan_segment_table_and_snapshot(&service, dataset_name, "simple_with_layers").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "simple_with_layers").await;
}

pub async fn register_and_scan_simple_dataset_multiple_timelines(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple_with_time_type(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
                TimeType::Sequence,
            ),
            LayerDefinition::simple_with_time_type(
                "my_segment_id2",
                &["my/entity"],
                TimeType::DurationNs,
            ),
            LayerDefinition::properties(
                "my_segment_id1",
                [prop(
                    "text_log",
                    re_sdk_types::archetypes::TextLog::new("i'm segment 1"),
                )],
            )
            .layer_name("props"),
            LayerDefinition::simple_with_time_type(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
                TimeType::TimestampNs,
            ),
            LayerDefinition::simple_with_time_type(
                "my_segment_id2",
                &["my/entity", "my/fourth/entity"],
                TimeType::Sequence,
            )
            .layer_name("layer_two"),
        ],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    scan_segment_table_and_snapshot(&service, dataset_name, "simple_with_multiple_timelines").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "simple_with_multiple_timelines")
        .await;
}

pub async fn register_with_prefix(fe: impl RerunCloudService) {
    let root_dir = tempfile::tempdir().expect("creating temp dir");

    // Note: for this test, we don't use teh `DataSourceDefinition` abstraction here because we need
    // tight control of where the RRDs are stored.
    let tuid_prefix1 = 1;
    create_simple_recording_in(
        tuid_prefix1,
        "my_segment_id1",
        &["my/entity", "my/other/entity"],
        TimeType::Sequence,
        root_dir.path(),
    )
    .expect("creating recording");

    let tuid_prefix2 = 2;
    create_simple_recording_in(
        tuid_prefix2,
        "my_segment_id2",
        &["my/entity"],
        TimeType::Sequence,
        root_dir.path(),
    )
    .expect("creating recording");

    let tuid_prefix3 = 3;
    create_simple_recording_in(
        tuid_prefix3,
        "my_segment_id3",
        &["my/entity", "another/one", "yet/another/one"],
        TimeType::Sequence,
        root_dir.path(),
    )
    .expect("creating recording");

    let dataset_name = "my_dataset1";
    fe.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
        name: Some(dataset_name.to_owned()),
        id: None,
    }))
    .await
    .unwrap();

    let root_url =
        Url::parse(&format!("file://{}/", root_dir.path().display())).expect("creating root url");

    fe.register_with_dataset_name_blocking(
        dataset_name,
        vec![
            DataSource {
                storage_url: Some(root_url.to_string()),
                prefix: true,
                layer: None,
                typ: DataSourceKind::Rrd as i32,
            }, //
        ],
    )
    .await;

    scan_segment_table_and_snapshot(&fe, dataset_name, "register_prefix_segments").await;
    scan_dataset_manifest_and_snapshot(&fe, dataset_name, "register_prefix_manifest").await;
}

// Scanning an empty dataset should return an empty dataframe with the expected schema -- not a
// NOT_FOUND error.
pub async fn register_and_scan_empty_dataset(service: impl RerunCloudService) {
    let dataset_name = "empty_dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    scan_segment_table_and_snapshot(&service, dataset_name, "empty").await;
    scan_dataset_manifest_and_snapshot(&service, dataset_name, "empty").await;
}

/// Any kind of bad file URI should return a not found error.
///
/// This includes:
/// - file not found
/// - path is not a file
/// - URI has a host name
///
/// The latter can be caused by attempting to build a `file://` with a relative path, leading to
/// `file://path/to/file.rrd`. This is valid URI, but here `path` is the hostname.
pub async fn register_bad_file_uri_should_error(service: impl RerunCloudService) {
    let temp_dir = tempfile::tempdir().expect("creating temp dir");
    let temp_dir_uri = format!("file://{}/", temp_dir.path().display());

    let test_cases = vec![
        ("file doesn't exist", "file:///does/not/exist.rrd"),
        ("URI has a host name", "file://somehost/file/path.rrd"),
        ("URI points to a directory", &temp_dir_uri),
    ];

    let dataset_name = "empty_dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    for (test_name, bad_uri) in test_cases {
        let request = RegisterWithDatasetRequest {
            data_sources: vec![ext::DataSource {
                storage_url: url::Url::parse(bad_uri).unwrap(),
                layer: "base".to_owned(),
                is_prefix: false,
                kind: ext::DataSourceKind::Rrd,
            }],
            on_duplicate: Default::default(),
        };

        let result = service
            .register_with_dataset(
                tonic::Request::new(request.into())
                    .with_entry_name(dataset_name)
                    .unwrap(),
            )
            .await;

        assert!(
            result.is_err(),
            "register on unknown file should fail (case: {test_name})"
        );
        assert_eq!(
            result.unwrap_err().code(),
            tonic::Code::NotFound,
            "bad file URI should result in a not found error (case: {test_name})"
        );
    }
}

pub async fn register_segment_bumps_timestamp(service: impl RerunCloudService) {
    async fn get_dataset_updated_at_nanos(
        service: &impl RerunCloudService,
        dataset_name: &str,
    ) -> i64 {
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

    let dataset_name = "timestamp_test_dataset";

    //
    // Create a dataset
    //

    service.create_dataset_entry_with_name(dataset_name).await;

    let initial_updated_at_nanos = get_dataset_updated_at_nanos(&service, dataset_name).await;

    // Small delay to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    //
    // Register a segment - this should update the timestamp
    //

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("segment1", &["my/entity"])],
    );

    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let after_register_updated_at_nanos =
        get_dataset_updated_at_nanos(&service, dataset_name).await;

    assert!(
        after_register_updated_at_nanos > initial_updated_at_nanos,
        "Timestamp should be updated after registering segment. Initial: {initial_updated_at_nanos}, After register: {after_register_updated_at_nanos}"
    );

    // Small delay to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    //
    // Register another layer to the same segment - this should also update the timestamp
    //

    let layer_data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        2,
        [LayerDefinition::simple("segment1", &["another/entity"]).layer_name("layer2")],
    );

    service
        .register_with_dataset_name_blocking(dataset_name, layer_data_sources_def.to_data_sources())
        .await;

    let after_layer_updated_at_nanos = get_dataset_updated_at_nanos(&service, dataset_name).await;

    assert!(
        after_layer_updated_at_nanos > after_register_updated_at_nanos,
        "Timestamp should be updated after adding a layer. After register: {after_register_updated_at_nanos}, After layer: {after_layer_updated_at_nanos}"
    );
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
        batch.format_schema_snapshot()
    );
    insta::assert_snapshot!(
        format!("{snapshot_name}_segments_data"),
        filtered_batch.format_snapshot(false)
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
