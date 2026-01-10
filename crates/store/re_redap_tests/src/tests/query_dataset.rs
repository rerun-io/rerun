use arrow::array::{FixedSizeBinaryArray, RecordBatch, RecordBatchOptions, UInt32Array};
use futures::StreamExt as _;
use re_log_types::{AbsoluteTimeRange, TimeInt};
use re_protos::cloud::v1alpha1::QueryDatasetResponse;
use re_protos::cloud::v1alpha1::ext::{
    DataSource, DataSourceKind, Query, QueryDatasetRequest, QueryLatestAt, QueryRange,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_types_core::ChunkId;

use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
};
use crate::{FieldsTestExt as _, RecordBatchTestExt as _, TempPath};

pub async fn query_empty_dataset(service: impl RerunCloudService) {
    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    query_dataset_snapshot(
        &service,
        QueryDatasetRequest::default(),
        &[],
        dataset_name,
        "empty_dataset",
    )
    .await;
}

pub async fn query_simple_dataset(service: impl RerunCloudService) {
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

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let requests = vec![
        (QueryDatasetRequest::default(), "default"),
        (
            QueryDatasetRequest {
                segment_ids: vec!["my_segment_id3".into()],
                ..Default::default()
            },
            "single_segment",
        ),
        (
            QueryDatasetRequest {
                entity_paths: vec!["/my/entity".into()],
                select_all_entity_paths: false,
                ..Default::default()
            },
            "single_entity",
        ),
        //TODO(RR-2613): add more test cases here when they are supported by OSS server
        (
            // Test exclude_static_data
            QueryDatasetRequest {
                exclude_static_data: true,
                ..Default::default()
            },
            "exclude_static",
        ),
        (
            // Test exclude_temporal_data
            QueryDatasetRequest {
                exclude_temporal_data: true,
                ..Default::default()
            },
            "exclude_temporal",
        ),
    ];

    for (request, snapshot_name) in requests {
        query_dataset_snapshot(
            &service,
            request,
            &[],
            dataset_name,
            &format!("simple_dataset_{snapshot_name}"),
        )
        .await;
    }
}

pub async fn query_simple_dataset_with_layers(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("partition1", &["my/entity"]),
            LayerDefinition::simple("partition1", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("partition2", &["another/one"]).layer_name("base"),
            LayerDefinition::simple("partition2", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("partition3", &["i/am/alone"]),
        ],
    );

    let dataset_name = "dataset_with_layers";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    query_dataset_snapshot(
        &service,
        QueryDatasetRequest::default(),
        &[],
        dataset_name,
        "simple_with_layer",
    )
    .await;
}

/// Test that failure cases return the correct error code.
pub async fn query_dataset_should_fail(service: impl RerunCloudService) {
    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;

    let test_cases = vec![
        (
            "cannot specify entity paths if `select_all_entity_paths` is true",
            QueryDatasetRequest {
                entity_paths: vec!["/entity/path".into()],
                select_all_entity_paths: true,
                ..Default::default()
            },
            tonic::Code::InvalidArgument,
        ),
        //TODO(#11591): add more failure cases
    ];

    for (descr, request, expected_code) in test_cases {
        let response = service
            .query_dataset(tonic::Request::new(request.into()))
            .await;

        match response {
            Ok(_) => {
                panic!("expected failure with code {expected_code}, but got success ({descr})",);
            }
            Err(err) => {
                assert_eq!(
                    err.code(),
                    expected_code,
                    "expected failure with code {expected_code}, but got {err} ({descr})"
                );
            }
        }
    }
}

//TODO(RR-2613): this recording needs fleshing out in order to test more interesting queries.
fn create_recording_for_query_testing() -> anyhow::Result<TempPath> {
    use re_chunk::{Chunk, TimePoint};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{EntityPath, TimeInt, build_frame_nr};
    use re_sdk::RecordingStreamBuilder;

    use crate::utils::rerun::{next_chunk_id_generator, next_row_id_generator};

    let segment_id = "static_test_segment";
    let tuid_prefix: u64 = 100;

    let tmp_dir = tempfile::tempdir()?;
    let tmp_path = tmp_dir.path().join(format!("{segment_id}.rrd"));

    let rec = RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    let frame0 = TimeInt::new_temporal(0);
    let points = MyPoint::from_iter(0..1);

    // /static_only: single MyPoint logged as static
    let static_only_chunk =
        Chunk::builder_with_id(next_chunk_id(), EntityPath::from("/static_only"))
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyPoints::descriptor_points(), Some(&points as _))],
            )
            .build()?;

    rec.send_chunk(static_only_chunk);

    // /both: MyPoint logged as static and another logged at frame = 0
    let both_static_chunk = Chunk::builder_with_id(next_chunk_id(), EntityPath::from("/both"))
        .with_sparse_component_batches(
            next_row_id(),
            TimePoint::default(),
            [(MyPoints::descriptor_points(), Some(&points as _))],
        )
        .build()?;
    rec.send_chunk(both_static_chunk);

    let both_temporal_chunk = Chunk::builder_with_id(next_chunk_id(), EntityPath::from("/both"))
        .with_sparse_component_batches(
            next_row_id(),
            [build_frame_nr(frame0)],
            [(MyPoints::descriptor_points(), Some(&points as _))],
        )
        .build()?;
    rec.send_chunk(both_temporal_chunk);

    // /temporal_only: MyPoint logged at frame = 0
    let temporal_only_chunk =
        Chunk::builder_with_id(next_chunk_id(), EntityPath::from("/temporal_only"))
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame0)],
                [(MyPoints::descriptor_points(), Some(&points as _))],
            )
            .build()?;
    rec.send_chunk(temporal_only_chunk);

    rec.flush_blocking()?;

    Ok(crate::TempPath::new(tmp_dir, tmp_path))
}

pub async fn query_dataset_with_various_queries(service: impl RerunCloudService) {
    let recording_path = create_recording_for_query_testing().unwrap();

    let dataset_name = "dataset_with_layers";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(
            dataset_name,
            vec![
                DataSource {
                    storage_url: url::Url::from_file_path(recording_path.as_path()).unwrap(),
                    is_prefix: false,
                    layer: "base".to_owned(),
                    kind: DataSourceKind::Rrd,
                }
                .into(),
            ],
        )
        .await;

    // TODO(RR-2613): we need considerably more use-cases here.
    let queries = [
        (None, vec![], "none"),
        (Some(Query::default()), vec![], "default"),
        (
            Some(Query {
                latest_at: Some(QueryLatestAt {
                    index: Some("frame_nr".to_owned()),
                    at: TimeInt::MAX,
                }),
                range: None,
                ..Default::default()
            }),
            vec![ChunkId::from_tuid(re_tuid::Tuid::from_nanos_and_inc(
                100, 3,
            ))],
            "latest_at_end",
        ),
        (
            Some(Query {
                latest_at: None,
                range: Some(QueryRange {
                    index: "frame_nr".to_owned(),
                    index_range: AbsoluteTimeRange {
                        min: TimeInt::MIN,
                        max: TimeInt::MAX,
                    },
                }),
                ..Default::default()
            }),
            vec![ChunkId::from_tuid(re_tuid::Tuid::from_nanos_and_inc(
                100, 3,
            ))],
            "range_all",
        ),
    ];

    for (query, chunk_ids_to_remove, snapshot_name) in queries {
        query_dataset_snapshot(
            &service,
            QueryDatasetRequest {
                segment_ids: vec![],
                chunk_ids: vec![],
                entity_paths: vec![],
                select_all_entity_paths: true,
                fuzzy_descriptors: vec![],
                exclude_static_data: false,
                exclude_temporal_data: false,
                scan_parameters: None,
                query,
            },
            &chunk_ids_to_remove,
            dataset_name,
            &format!("with_query_{snapshot_name}"),
        )
        .await;
    }
}

// ---

// TODO(rerun-io/dataplatform#2228) remove the `chunk_ids_to_remove` parameter
async fn query_dataset_snapshot(
    service: &impl RerunCloudService,
    query_dataset_request: QueryDatasetRequest,
    chunk_ids_to_remove: &[ChunkId],
    dataset_name: &str,
    snapshot_name: &str,
) {
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
    let merged_chunk_info =
        remove_rows_containing_chunk_id(&merged_chunk_info, chunk_ids_to_remove);

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
    let required_chunk_info = merged_chunk_info.project_columns(&required_column_names);

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

/// Utility function to removes specific rows from a record batch. Because
/// correctness only requires that a minimal chunks are returned, it is
/// acceptable for additional chunks to be included in query results. While
/// not optimal, this function allows us to test for correctness while
/// we make improvements in performance.
fn remove_rows_containing_chunk_id(
    rb: &RecordBatch,
    chunk_ids: &[re_types_core::ChunkId],
) -> RecordBatch {
    let chunk_id_col = rb
        .column_by_name("chunk_id")
        .expect("Missing column chunk_id");

    let chunk_id_array = chunk_id_col
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .expect("chunk_id is not FixedSizeBinary");
    let chunk_id_slice = re_types_core::ChunkId::try_slice_from_arrow(chunk_id_array)
        .expect("chunk_id column should be convertible to ChunkId slice");

    let mut indices_to_keep = Vec::new();

    for (row_idx, chunk_id) in chunk_id_slice.iter().enumerate() {
        if !chunk_ids.contains(chunk_id) {
            indices_to_keep.push(row_idx as u32);
        }
    }

    let indices = UInt32Array::from(indices_to_keep);

    let resultant_rows = arrow::compute::take_arrays(rb.columns(), &indices, None)
        .expect("take_arrays should return arrays");

    RecordBatch::try_new_with_options(rb.schema(), resultant_rows, &RecordBatchOptions::default())
        .expect("should create record batch")
}
