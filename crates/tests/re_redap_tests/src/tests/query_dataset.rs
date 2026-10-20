use std::collections::{BTreeMap, BTreeSet};

use arrow::array::{RecordBatch, RecordBatchOptions, UInt32Array};
use futures::StreamExt as _;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt};
use re_protos::cloud::v1alpha1::FetchChunksRequest;
use re_protos::cloud::v1alpha1::ext::{
    ChunkCandidatesDataframe, ChunkMode, ComponentSelection, ContentSelection, DataSource,
    DirectUrlMode, EntitySelection, FillMode, FindChunksRequest, Query, QueryDatasetDataframe,
    QueryDatasetRequest, QueryLatestAt, QueryRange, RangeSelection, SegmentSelection,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_types_core::{ChunkId, SegmentId};

use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
    entry_name,
};
use crate::utils::{
    register_layered_dataset, register_mixed_timelines_dataset, register_per_segment_dataset,
    register_simple_dataset,
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
    let dataset_name = "dataset";
    let _data_sources_def = register_simple_dataset(&service, dataset_name).await;

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
    let dataset_name = "dataset_with_layers";
    let _data_sources_def = register_layered_dataset(&service, dataset_name).await;

    query_dataset_snapshot(
        &service,
        QueryDatasetRequest::default(),
        &[],
        dataset_name,
        "simple_with_layer",
    )
    .await;
}

/// Querying with a `segment_id` that does not exist must NOT error — it must
/// succeed and return zero rows for that segment. Mixed requests (one real,
/// one unknown) must succeed and return rows only for the real segment.
///
/// Rationale: `segment_ids` is also what DataFusion filter pushdown produces
/// for `WHERE rerun_segment_id = 'foo'`. The value is data, not a referent.
/// Erroring would turn ordinary SQL filters that happen not to match into
/// hand-grenades. Explicit-API callers (`filter_segments`,
/// `using_index_values`) accept the same silent-ignore semantics — the
/// alternative is a roundtrip to validate IDs, which is exactly the
/// expensive call we're trying to avoid.
pub async fn query_dataset_unknown_segment_id_returns_empty(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("real_segment", &["my/entity"])],
    );

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    // (description, request segment_ids, expected: any data rows?)
    let test_cases = [
        ("only unknown", vec!["doesnt_exist".into()], false),
        (
            "mixed real + unknown",
            vec!["real_segment".into(), "doesnt_exist".into()],
            true,
        ),
    ];

    for (descr, segment_ids, expect_rows) in test_cases {
        let chunk_info: Vec<RecordBatch> = service
            .query_dataset(
                tonic::Request::new(
                    QueryDatasetRequest {
                        segment_ids,
                        ..Default::default()
                    }
                    .into(),
                )
                .with_entry_name(entry_name(dataset_name)),
            )
            .await
            .unwrap_or_else(|err| panic!("query_dataset must succeed ({descr}): {err}"))
            .into_inner()
            .flat_map(|resp| {
                futures::stream::iter(
                    resp.unwrap_or_else(|err| {
                        panic!("query_dataset stream must not error ({descr}): {err}")
                    })
                    .data,
                )
            })
            .map(|dfp| dfp.try_into().unwrap())
            .collect()
            .await;

        let merged = concat_record_batches(&chunk_info);
        let has_rows = merged.num_rows() > 0;
        assert_eq!(
            has_rows,
            expect_rows,
            "unexpected row presence for {descr}: got {} rows",
            merged.num_rows(),
        );
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

    let rec = RecordingStreamBuilder::new(
        re_log_types::ApplicationId::try_new(format!("rerun_example_{segment_id}")).unwrap(),
    )
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
                DataSource::new_rrd_url(
                    url::Url::from_file_path(recording_path.as_path()).unwrap(),
                    None,
                )
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
                latest_at: None,
                range: Some(QueryRange {
                    index: "frame_nr".into(),
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
                generate_direct_urls: false,
                unsigned_direct_urls: false,
            },
            &chunk_ids_to_remove,
            dataset_name,
            &format!("with_query_{snapshot_name}"),
        )
        .await;
    }
}

/// Verify that `chunk_byte_size_uncompressed` is present and populated with
/// non-zero values for every chunk in the query response.
pub async fn query_dataset_has_uncompressed_sizes(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("segment", &["my/entity"])],
    );

    let dataset_name = "dataset";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let chunk_info: Vec<RecordBatch> = service
        .query_dataset(
            tonic::Request::new(QueryDatasetRequest::default().into())
                .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap()
        .into_inner()
        .flat_map(|resp| futures::stream::iter(resp.unwrap().data))
        .map(|dfp| dfp.try_into().unwrap())
        .collect()
        .await;

    let merged = concat_record_batches(&chunk_info);
    assert!(
        merged.num_rows() > 0,
        "query should return at least one chunk"
    );

    let uncompressed = QueryDatasetDataframe::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED
        .extract(&merged)
        .expect("chunk_byte_size_uncompressed column must be present");

    for (i, size) in uncompressed.iter().enumerate() {
        let size = size.unwrap_or_else(|| panic!("row {i}: uncompressed size must not be null"));
        assert!(0 < size, "row {i}: uncompressed size must be > 0");
    }
}

/// `QueryDataset` is deprecated, but released rerun SDKs still send it. For each wire shape those
/// SDKs produce, the response must name the same chunks as a `FindChunks` request built by hand
/// for the same selection.
pub async fn query_dataset_matches_find_chunks(service: impl RerunCloudService) {
    let _per_segment_ranges = register_per_segment_dataset(
        &service,
        "per_segment_ranges",
        900,
        re_log_types::TimeType::Sequence,
    )
    .await;
    let _mixed_timelines_dataset =
        register_mixed_timelines_dataset(&service, "mixed_timelines").await;

    let required_base_columns = QueryDatasetDataframe::min_schema()
        .fields()
        .iter()
        .map(|field| field.name().to_owned())
        .collect::<Vec<_>>();

    for case in matches_find_chunks_cases() {
        let EquivalentRequests {
            name,
            dataset,
            timeline,
            query_dataset,
            find_chunks,
        } = case;

        let query_dataset_response = query_dataset_batches(&service, dataset, query_dataset).await;
        assert!(
            !query_dataset_response.is_empty(),
            "case `{name}`: query_dataset must return at least one batch"
        );
        for (idx, batch) in query_dataset_response.iter().enumerate() {
            let schema = batch.schema();
            for column_name in &required_base_columns {
                assert!(
                    schema.field_with_name(column_name).is_ok(),
                    "case `{name}`, batch {idx}: missing base column `{column_name}`"
                );
            }
            if let Some(timeline) = timeline {
                let start_column = format!("{timeline}:start");
                assert!(
                    schema.field_with_name(&start_column).is_ok(),
                    "case `{name}`, batch {idx}: request names timeline `{timeline}`, expected `{start_column}`"
                );
            }
        }

        let find_chunks_response = find_chunks_batches(&service, dataset, find_chunks).await;

        assert_eq!(
            query_dataset_chunk_ids(&query_dataset_response),
            find_chunks_chunk_ids(&find_chunks_response),
            "case `{name}`: query_dataset and find_chunks must return the same chunk_id set"
        );
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
                .with_entry_name(entry_name(dataset_name)),
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
    let required_field = QueryDatasetDataframe::min_schema().fields().to_vec();

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
            QueryDatasetDataframe::COLUMN_CHUNK_KEY_NAME,
            QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN_NAME,
            QueryDatasetDataframe::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED_NAME,
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
    let chunk_id_col = QueryDatasetDataframe::COLUMN_CHUNK_ID
        .extract(rb)
        .expect("bad chunk_id column");

    let mut indices_to_keep = Vec::new();

    for (row_idx, chunk_id) in chunk_id_col.iter_owned().enumerate() {
        if !chunk_ids.contains(&chunk_id) {
            indices_to_keep.push(row_idx as u32);
        }
    }

    let indices = UInt32Array::from(indices_to_keep);

    let resultant_rows = arrow::compute::take_arrays(rb.columns(), &indices, None)
        .expect("take_arrays should return arrays");

    RecordBatch::try_new_with_options(rb.schema(), resultant_rows, &RecordBatchOptions::default())
        .expect("should create record batch")
}

/// Entity shared by every fixture [`query_dataset_matches_find_chunks`] uses.
const ENTITY: &str = "my/entity";

/// Timeline shared by every fixture except the mixed-timeline one.
const TIMELINE: &str = "frame_nr";

/// A `QueryDataset` request and a `FindChunks` request for the same selection on one dataset.
struct EquivalentRequests {
    name: &'static str,
    dataset: &'static str,

    /// The timeline the `QueryDataset` request names, if any. Its response must carry
    /// `{timeline}:start`.
    timeline: Option<&'static str>,
    query_dataset: QueryDatasetRequest,
    find_chunks: FindChunksRequest,
}

fn scan_parameters() -> ScanParameters {
    ScanParameters {
        columns: FetchChunksRequest::required_column_names(),
        ..Default::default()
    }
}

/// Builds a `QueryDataset` request as the SDK's planner does: explicit entities unless `select_all`,
/// and the `scan_parameters`/`columns_always_include_byte_offsets` every production request
/// carries (`query_from_query_expression` always sets the latter).
#[expect(clippy::fn_params_excessive_bools)] // test helper, positional booleans read fine here
fn sdk_query_dataset_request(
    segment_ids: Vec<SegmentId>,
    select_all: bool,
    fuzzy_descriptors: Vec<String>,
    query: Query,
    generate_direct_urls: bool,
) -> QueryDatasetRequest {
    QueryDatasetRequest {
        segment_ids,
        entity_paths: if select_all {
            vec![]
        } else {
            vec![EntityPath::from(ENTITY)]
        },
        select_all_entity_paths: select_all,
        fuzzy_descriptors,
        scan_parameters: Some(scan_parameters()),
        query: Some(Query {
            columns_always_include_byte_offsets: true,
            ..query
        }),
        generate_direct_urls,
        ..Default::default()
    }
}

fn static_query() -> Query {
    Query {
        latest_at: Some(QueryLatestAt::new_static()),
        range: None,
        ..Default::default()
    }
}

fn full_scan_query() -> Query {
    Query {
        latest_at: None,
        range: None,
        ..Default::default()
    }
}

fn range_query(
    timeline: &'static str,
    min: TimeInt,
    max: TimeInt,
    fill_at: Option<TimeInt>,
) -> Query {
    Query {
        latest_at: fill_at.map(|at| QueryLatestAt::global(Some(timeline.into()), at)),
        range: Some(QueryRange {
            index: timeline.into(),
            index_range: AbsoluteTimeRange { min, max },
        }),
        ..Default::default()
    }
}

fn samples_query(timeline: &'static str, per_segment_values: Vec<Vec<i64>>) -> Query {
    Query {
        latest_at: Some(QueryLatestAt {
            index: Some(timeline.into()),
            at: TimeInt::STATIC,
            per_segment_values,
        }),
        range: None,
        ..Default::default()
    }
}

fn content_all() -> ContentSelection {
    ContentSelection::All
}

fn content_explicit(components: ComponentSelection) -> ContentSelection {
    ContentSelection::Explicit(vec![EntitySelection {
        entity_path: EntityPath::from(ENTITY),
        components,
    }])
}

fn find_static(
    segments: SegmentSelection,
    contents: ContentSelection,
    omit_direct_urls: bool,
) -> FindChunksRequest {
    FindChunksRequest {
        contents,
        mode: ChunkMode::Static { segments },
        direct_url_mode: if omit_direct_urls {
            DirectUrlMode::Omit
        } else {
            DirectUrlMode::Signed
        },
    }
}

fn find_range(
    timeline: &'static str,
    start_inclusive: Option<TimeInt>,
    end_inclusive: Option<TimeInt>,
    fill_mode: FillMode,
    segments: SegmentSelection,
    contents: ContentSelection,
    omit_direct_urls: bool,
) -> FindChunksRequest {
    FindChunksRequest {
        contents,
        mode: ChunkMode::Range {
            segments,
            range: RangeSelection {
                timeline: timeline.into(),
                start_inclusive,
                end_inclusive,
                fill_mode,
            },
        },
        direct_url_mode: if omit_direct_urls {
            DirectUrlMode::Omit
        } else {
            DirectUrlMode::Signed
        },
    }
}

fn find_samples(
    timeline: &'static str,
    segments: BTreeMap<SegmentId, BTreeSet<TimeInt>>,
    contents: ContentSelection,
    omit_direct_urls: bool,
) -> FindChunksRequest {
    FindChunksRequest {
        contents,
        mode: ChunkMode::Samples {
            timeline: timeline.into(),
            segments,
            fill_mode: FillMode::LatestAt,
        },
        direct_url_mode: if omit_direct_urls {
            DirectUrlMode::Omit
        } else {
            DirectUrlMode::Signed
        },
    }
}

fn seg(id: &str) -> SegmentId {
    SegmentId::from(id)
}

fn matches_find_chunks_cases() -> Vec<EquivalentRequests> {
    const RANGES: &str = "per_segment_ranges";
    const MIXED_TIMELINES: &str = "mixed_timelines";

    vec![
        // `reader(index=None)`: Static, dataset scope (empty `segment_ids` means All).
        EquivalentRequests {
            name: "static_dataset_scope",
            dataset: RANGES,
            timeline: None,
            query_dataset: sdk_query_dataset_request(vec![], false, vec![], static_query(), false),
            find_chunks: find_static(
                SegmentSelection::All,
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // `query: None` or an empty `Query` names no timeline, but a `FindChunks` Range must. The
        // fixture's segments use different timelines, so this shows that the unbounded Range does
        // not narrow the result to its timeline.
        EquivalentRequests {
            name: "timeline_less_full_scan",
            dataset: MIXED_TIMELINES,
            timeline: None,
            query_dataset: sdk_query_dataset_request(
                vec![],
                false,
                vec![],
                full_scan_query(),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                None,
                None,
                FillMode::None,
                SegmentSelection::All,
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // `reader(index=T)` with a bounded filter, segment scope, `generate_direct_urls = false`.
        EquivalentRequests {
            name: "bounded_range_segment_scope",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![seg("seg1")],
                false,
                vec![],
                range_query(
                    TIMELINE,
                    TimeInt::new_temporal(1010),
                    TimeInt::new_temporal(1030),
                    None,
                ),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                Some(TimeInt::new_temporal(1010)),
                Some(TimeInt::new_temporal(1030)),
                FillMode::None,
                SegmentSelection::Explicit(BTreeSet::from([seg("seg1")])),
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // Same bounded range, `generate_direct_urls = true`: exercises both signing values on one
        // populated Range row.
        EquivalentRequests {
            name: "bounded_range_direct_urls",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![seg("seg1")],
                false,
                vec![],
                range_query(
                    TIMELINE,
                    TimeInt::new_temporal(1010),
                    TimeInt::new_temporal(1030),
                    None,
                ),
                true,
            ),
            find_chunks: find_range(
                TIMELINE,
                Some(TimeInt::new_temporal(1010)),
                Some(TimeInt::new_temporal(1030)),
                FillMode::None,
                SegmentSelection::Explicit(BTreeSet::from([seg("seg1")])),
                content_explicit(ComponentSelection::All),
                false,
            ),
        },
        // `T > t` / `T >= t`: lower-only range, no fill. Strict and inclusive inequalities share
        // this wire shape, so one row covers both.
        EquivalentRequests {
            name: "lower_only_range",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![],
                false,
                vec![],
                range_query(TIMELINE, TimeInt::new_temporal(1020), TimeInt::MAX, None),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                Some(TimeInt::new_temporal(1020)),
                None,
                FillMode::None,
                SegmentSelection::All,
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // `T < t` / `T <= t`: upper-only range, no fill (no AND merge here, so no `latest_at`).
        EquivalentRequests {
            name: "upper_only_range",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![],
                false,
                vec![],
                range_query(TIMELINE, TimeInt::MIN, TimeInt::new_temporal(1030), None),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                None,
                Some(TimeInt::new_temporal(1030)),
                FillMode::None,
                SegmentSelection::All,
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // Range with fill, upper bound explicitly at `MAX` (not omitted): proves the literal
        // endpoint round-trips distinctly from an unbounded upper side.
        EquivalentRequests {
            name: "range_with_fill_upper_bound_at_max",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![],
                false,
                vec![],
                range_query(
                    TIMELINE,
                    TimeInt::new_temporal(1010),
                    TimeInt::MAX,
                    Some(TimeInt::new_temporal(1010)),
                ),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                Some(TimeInt::new_temporal(1010)),
                Some(TimeInt::MAX),
                FillMode::LatestAt,
                SegmentSelection::All,
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // Equal-bound range at the lower domain extreme, no fill: a legacy degenerate `[at, at]`.
        EquivalentRequests {
            name: "equal_bound_min_no_fill",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![],
                false,
                vec![],
                range_query(TIMELINE, TimeInt::MIN, TimeInt::MIN, None),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                Some(TimeInt::MIN),
                Some(TimeInt::MIN),
                FillMode::None,
                SegmentSelection::All,
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // Equal-bound range at the upper domain extreme, with fill: a legacy degenerate `[at, at]`
        // paired with a matching `latest_at(at)`, so the before/after comparison stays valid.
        EquivalentRequests {
            name: "equal_bound_max_with_fill",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![],
                false,
                vec![],
                range_query(TIMELINE, TimeInt::MAX, TimeInt::MAX, Some(TimeInt::MAX)),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                Some(TimeInt::MAX),
                Some(TimeInt::MAX),
                FillMode::LatestAt,
                SegmentSelection::All,
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // `using_index_values={seg: values}`: per-segment sample lists.
        EquivalentRequests {
            name: "samples",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![seg("seg1"), seg("seg2")],
                false,
                vec![],
                samples_query(TIMELINE, vec![vec![1010, 1020], vec![2010]]),
                false,
            ),
            find_chunks: find_samples(
                TIMELINE,
                BTreeMap::from([
                    (
                        seg("seg1"),
                        BTreeSet::from([TimeInt::new_temporal(1010), TimeInt::new_temporal(1020)]),
                    ),
                    (seg("seg2"), BTreeSet::from([TimeInt::new_temporal(2010)])),
                ]),
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // One segment's value list is empty: that segment contributes no candidates, so only
        // `seg1`'s samples remain.
        EquivalentRequests {
            name: "samples_one_segment_empty",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![seg("seg1"), seg("seg2")],
                false,
                vec![],
                samples_query(TIMELINE, vec![vec![1010, 1020], vec![]]),
                false,
            ),
            find_chunks: find_samples(
                TIMELINE,
                BTreeMap::from([(
                    seg("seg1"),
                    BTreeSet::from([TimeInt::new_temporal(1010), TimeInt::new_temporal(1020)]),
                )]),
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // Every segment's value list is empty: the request selects no chunks, so both servers
        // answer with one empty batch.
        EquivalentRequests {
            name: "samples_all_segments_empty",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![seg("seg1"), seg("seg2")],
                false,
                vec![],
                samples_query(TIMELINE, vec![vec![], vec![]]),
                false,
            ),
            find_chunks: find_samples(
                TIMELINE,
                BTreeMap::new(),
                content_explicit(ComponentSelection::All),
                true,
            ),
        },
        // Projection/filter narrows components to a flat `fuzzy_descriptors` list: only the
        // static chunk carries `labels`, so this also proves component narrowing excludes the
        // temporal chunks rather than being a no-op.
        EquivalentRequests {
            name: "exact_components_static",
            dataset: RANGES,
            timeline: None,
            query_dataset: sdk_query_dataset_request(
                vec![],
                false,
                vec![labels_component_name()],
                static_query(),
                false,
            ),
            find_chunks: find_static(
                SegmentSelection::All,
                content_explicit(ComponentSelection::Explicit(BTreeSet::from([
                    labels_component_name(),
                ]))),
                true,
            ),
        },
        // A projected `reader(index=T)` without a time filter: released SDKs send the projected
        // components without their entity (RR-5816). The server lists the dataset's entities, so
        // the components still narrow the result.
        EquivalentRequests {
            name: "select_all_with_components",
            dataset: RANGES,
            timeline: None,
            query_dataset: sdk_query_dataset_request(
                vec![],
                true,
                vec![labels_component_name()],
                full_scan_query(),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                None,
                None,
                FillMode::None,
                SegmentSelection::All,
                content_explicit(ComponentSelection::Explicit(BTreeSet::from([
                    labels_component_name(),
                ]))),
                true,
            ),
        },
        // The viewer's manifest-less fallback: `select_all_entity_paths = true` and
        // `Query::latest_at_range`, which pairs `at` with `range.min`.
        EquivalentRequests {
            name: "viewer_latest_at_range",
            dataset: RANGES,
            timeline: Some(TIMELINE),
            query_dataset: sdk_query_dataset_request(
                vec![],
                true,
                vec![],
                Query::latest_at_range(
                    TIMELINE.into(),
                    AbsoluteTimeRange {
                        min: TimeInt::new_temporal(1010),
                        max: TimeInt::new_temporal(1040),
                    },
                ),
                false,
            ),
            find_chunks: find_range(
                TIMELINE,
                Some(TimeInt::new_temporal(1010)),
                Some(TimeInt::new_temporal(1040)),
                FillMode::LatestAt,
                SegmentSelection::All,
                content_all(),
                true,
            ),
        },
    ]
}

/// The `labels` component's identifier, as `fuzzy_descriptors`/`ComponentSelection` name it: the
/// only component on the static chunk `create_simple_recording_one_chunk_per_frame` emits, so
/// selecting it alone excludes every temporal chunk.
fn labels_component_name() -> String {
    use re_log_types::example_components::MyPoints;
    MyPoints::descriptor_labels().display_name().to_owned()
}

async fn query_dataset_batches<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    request: QueryDatasetRequest,
) -> Vec<RecordBatch> {
    let wire: re_protos::cloud::v1alpha1::QueryDatasetRequest = request.into();
    service
        .query_dataset(tonic::Request::new(wire).with_entry_name(entry_name(dataset_name)))
        .await
        .unwrap_or_else(|err| panic!("query_dataset must succeed: {err}"))
        .into_inner()
        .map(|resp| resp.unwrap_or_else(|err| panic!("query_dataset stream must not error: {err}")))
        .filter_map(|resp| futures::future::ready(resp.data))
        .map(|dfp| dfp.try_into().expect("valid record batch"))
        .collect()
        .await
}

async fn find_chunks_batches<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    request: FindChunksRequest,
) -> Vec<RecordBatch> {
    let wire: re_protos::cloud::v1alpha1::FindChunksRequest = request.into();
    service
        .find_chunks(tonic::Request::new(wire).with_entry_name(entry_name(dataset_name)))
        .await
        .unwrap_or_else(|err| panic!("find_chunks must succeed: {err}"))
        .into_inner()
        .map(|resp| resp.unwrap_or_else(|err| panic!("find_chunks stream must not error: {err}")))
        .filter_map(|resp| futures::future::ready(resp.data))
        .map(|dfp| dfp.try_into().expect("valid record batch"))
        .collect()
        .await
}

fn query_dataset_chunk_ids(batches: &[RecordBatch]) -> BTreeSet<ChunkId> {
    let mut ids = BTreeSet::new();
    for batch in batches {
        let column = QueryDatasetDataframe::COLUMN_CHUNK_ID
            .extract(batch)
            .expect("bad chunk_id column in query_dataset response");
        ids.extend(column.iter_owned());
    }
    ids
}

fn find_chunks_chunk_ids(batches: &[RecordBatch]) -> BTreeSet<ChunkId> {
    let mut ids = BTreeSet::new();
    for batch in batches {
        let column = ChunkCandidatesDataframe::COLUMN_CHUNK_ID
            .extract(batch)
            .expect("bad chunk_id column in find_chunks response");
        ids.extend(column.iter_owned());
    }
    ids
}
