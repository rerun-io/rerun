use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::array::RecordBatch;
use datafusion::datasource::TableProvider as _;
use datafusion::physical_plan::ExecutionPlanProperties as _;
use datafusion::prelude::SessionContext;
use futures::{StreamExt as _, TryStreamExt as _};
use itertools::Itertools as _;
use re_chunk_store::IndexValue;
use re_datafusion::{DataframeQueryTableProvider, NoOpObjectStoreAuthenticator};
use re_log_types::{EntityPath, TimeInt, TimeType};
use re_protos::cloud::v1alpha1::ext;
use re_protos::cloud::v1alpha1::ext::DatasetEntry;
use re_protos::cloud::v1alpha1::ext::QueryDatasetDataframe;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_types_core::SegmentId;

use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches, prop,
};
use crate::utils::client::{RecordedRequest, TestClient};
use crate::utils::register_per_segment_dataset;

pub async fn query_dataset_index_values_by_time_type<T: RerunCloudService>(
    service: Arc<T>,
    time_type: TimeType,
) {
    query_dataset_index_values_by_time_type_impl(service, time_type, true).await;
}

async fn query_dataset_index_values_by_time_type_impl<T: RerunCloudService>(
    service: Arc<T>,
    time_type: TimeType,
    supports_find_chunks: bool,
) {
    let tuid_prefix = match time_type {
        TimeType::TimestampNs => 1,
        TimeType::DurationNs => 10,
        TimeType::Sequence => 20,
    };

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        tuid_prefix,
        [
            LayerDefinition::simple_with_time(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
                1000,
                time_type,
            ),
            LayerDefinition::simple_with_time("my_segment_id2", &["my/entity"], 2000, time_type),
            LayerDefinition::properties(
                "my_segment_id1",
                [prop(
                    "text_log",
                    re_sdk_types::archetypes::TextLog::new("i'm segment 1"),
                )],
            )
            .layer_name("props"),
            LayerDefinition::simple_with_time(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
                3000,
                time_type,
            ),
        ],
    );

    let dataset_name = format!("dataset_{time_type}");
    let dataset_entry = service.create_dataset_entry_with_name(&dataset_name).await;
    service
        .register_with_dataset_name_blocking(&dataset_name, data_sources_def.to_data_sources())
        .await;

    let client = TestClient::new(service);

    let tests = vec![
        (
            vec![
                ("my_segment_id1", vec![1020, 1040]),
                ("my_segment_id2", vec![2010, 2030]),
                ("my_segment_id3", vec![3010, 3020, 3030, 3040]),
            ],
            "all_valid_index_values",
            true,
        ),
        (
            vec![("my_segment_id1", vec![1020, 1040])],
            "single_segment",
            false,
        ),
        (
            vec![("my_segment_id4", vec![1020, 1040])],
            "unknown_segment",
            false,
        ),
    ];

    for (index_values, snapshot_name, check_schema) in tests {
        query_dataset_snapshot(
            client.clone(),
            &dataset_entry,
            index_values,
            &format!("query_index_values_{time_type}_{snapshot_name}"),
            time_type,
            QueryDatasetSnapshotOptions {
                check_schema,
                supports_find_chunks,
            },
        )
        .await;
    }
}

pub async fn query_dataset_index_values(service: impl RerunCloudService) {
    let service = Arc::new(service);
    query_dataset_index_values_by_time_type(service.clone(), TimeType::Sequence).await;
    query_dataset_index_values_by_time_type(service.clone(), TimeType::DurationNs).await;
    query_dataset_index_values_by_time_type(service.clone(), TimeType::TimestampNs).await;
}

/// [`query_dataset_index_values`] on the SDK's `query_dataset` path: the same fixtures and
/// snapshots, forced onto the `query_dataset` RPC via
/// `DataframeQueryTableProvider::with_find_chunks_support(false)`.
/// Both servers still implement the old RPC through their adapters, so the same fixtures serve;
/// the snapshot names are reused verbatim, so a divergence between the two paths shows up as a
/// snapshot mismatch.
///
/// TODO(RR-5817): remove with the SDK's `query_dataset` path.
pub async fn query_dataset_index_values_query_dataset_path(service: impl RerunCloudService) {
    let service = Arc::new(service);
    query_dataset_index_values_by_time_type_impl(service.clone(), TimeType::Sequence, false).await;
    query_dataset_index_values_by_time_type_impl(service.clone(), TimeType::DurationNs, false)
        .await;
    query_dataset_index_values_by_time_type_impl(service.clone(), TimeType::TimestampNs, false)
        .await;
}

/// Collect the set of chunk IDs returned by a query.
///
/// Lets us assert that `per_segment_values` actually narrows the result
/// (a strict subset of the baseline chunk-id set) instead of just
/// counting rows, which can pass even when the server ignores the
/// per-segment filter and returns the full baseline.
async fn per_segment_chunk_id_set<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    request: ext::QueryDatasetRequest,
) -> BTreeSet<re_chunk::ChunkId> {
    use re_protos::cloud::v1alpha1::QueryDatasetResponse;
    use re_protos::headers::RerunHeadersInjectorExt as _;

    let stream = service
        .query_dataset(
            tonic::Request::new(request.into())
                .with_entry_name(crate::tests::common::entry_name(dataset_name)),
        )
        .await
        .unwrap()
        .into_inner();
    let mut stream = Box::pin(stream);

    let mut ids: BTreeSet<re_chunk::ChunkId> = BTreeSet::new();
    while let Some(resp) = stream.next().await {
        let resp: QueryDatasetResponse = resp.unwrap();
        if let Some(part) = resp.data {
            let batch: arrow::array::RecordBatch = part.try_into().unwrap();
            let id_col = QueryDatasetDataframe::COLUMN_CHUNK_ID
                .extract(&batch)
                .expect("bad chunk_id column in response");
            ids.extend(id_col.iter_owned());
        }
    }
    ids
}

fn per_segment_segment_ids() -> Vec<re_types_core::SegmentId> {
    vec!["seg1".into(), "seg2".into(), "seg3".into()]
}

/// Wire-level test for `QueryLatestAt.per_segment_values`.
///
/// Builds a dataset with 3 segments, each holding 4 single-frame temporal
/// chunks (`start_time + {10, 20, 30, 40}`) plus a static chunk, then issues
/// two `QueryDatasetRequest`s directly via the gRPC layer:
///
/// 1. `range_baseline`: a `range(EVERYTHING)` query — every temporal chunk
///    plus statics. Used as the strict-subset superset.
/// 2. `filtered`: `per_segment_values` carrying a single value per segment.
///    Must be a strict subset of `range_baseline_ids` (proves push-down
///    actually narrowed).
///
/// This is the regression-guard for the per-segment chunk filter on every
/// backend: the OSS server's per-row filter and the Rerun Hub's
/// manifest-based virtual `ChunkStore` resolution both have to honor it.
pub async fn query_dataset_per_segment_values_wire_level(service: impl RerunCloudService) {
    let service = Arc::new(service);
    for time_type in [
        TimeType::Sequence,
        TimeType::DurationNs,
        TimeType::TimestampNs,
    ] {
        query_dataset_per_segment_values_wire_level_by_time_type(service.as_ref(), time_type).await;
    }
}

async fn query_dataset_per_segment_values_wire_level_by_time_type<T: RerunCloudService>(
    service: &T,
    time_type: TimeType,
) {
    use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;

    let dataset_name = &format!("per_segment_wire_level_{time_type}");
    let tuid_prefix = match time_type {
        TimeType::Sequence => 77,
        TimeType::DurationNs => 78,
        TimeType::TimestampNs => 79,
    };
    let _data_sources =
        register_per_segment_dataset(service, dataset_name, tuid_prefix, time_type).await;

    let index_name = re_log_types::build_index_value(0_i64, time_type)
        .0
        .name()
        .as_str();

    let segment_ids = per_segment_segment_ids();

    // 1. range(EVERYTHING) baseline — superset of every other query.
    let range_baseline_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(ext::Query {
            range: Some(ext::QueryRange {
                index: index_name.into(),
                index_range: re_log_types::AbsoluteTimeRange::EVERYTHING,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let range_baseline_ids =
        per_segment_chunk_id_set(service, dataset_name, range_baseline_request).await;
    assert!(
        !range_baseline_ids.is_empty(),
        "range baseline must return at least one chunk, got 0"
    );

    // 2. Filtered: per_segment_values with one value per segment that matches
    //    a single temporal chunk (frame `start + 10`).
    let filtered_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(ext::Query {
            latest_at: Some(ext::QueryLatestAt {
                index: Some(index_name.into()),
                // `at` is the global fallback; servers prefer per_segment_values when set.
                at: TimeInt::STATIC,
                per_segment_values: vec![
                    vec![1010], // seg1 (start_time 1000)
                    vec![2010], // seg2 (start_time 2000)
                    vec![3010], // seg3 (start_time 3000)
                ],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let filtered_ids = per_segment_chunk_id_set(service, dataset_name, filtered_request).await;
    assert!(
        !filtered_ids.is_empty(),
        "per_segment_values filter must still return matched chunks, got 0"
    );

    // The per-segment filter must actually narrow: the resulting chunk-id set
    // has to be a strict subset of the full-range baseline.
    assert!(
        filtered_ids.is_subset(&range_baseline_ids),
        "per_segment_values must only return chunks already in the range baseline; \
         got {} extra chunk(s)",
        filtered_ids.difference(&range_baseline_ids).count(),
    );
    assert!(
        filtered_ids.len() < range_baseline_ids.len(),
        "per_segment_values must narrow the result set strictly below the range baseline; \
         range_baseline={} filtered={}",
        range_baseline_ids.len(),
        filtered_ids.len(),
    );
}

/// The client-side pushdown: a scoped
/// `using_index_values` reader must emit `QueryLatestAt.per_segment_values`
/// aligned to its `segment_ids`.
///
/// Unlike the wire-level tests above, this drives the real
/// `DataframeQueryTableProvider::scan` and inspects the request it emits via
/// [`TestClient`]'s `recorded_requests`, rather than hand-building the request.
///
/// The reader has no fill, so the request must not ask for latest-at chunks.
///
/// Runs for every [`TimeType`] (`Sequence`, `DurationNs`, `TimestampNs`) since
/// the pushed-down index is timeline-typed.
pub async fn query_dataset_emits_per_segment_pushdown(service: impl RerunCloudService) {
    let service = Arc::new(service);
    for time_type in [
        TimeType::Sequence,
        TimeType::DurationNs,
        TimeType::TimestampNs,
    ] {
        query_dataset_emits_per_segment_pushdown_by_time_type(&service, time_type, true).await;
    }
}

/// [`query_dataset_emits_per_segment_pushdown`] on the SDK's `query_dataset` path: the same
/// fixture and pushdown assertions, forced onto the `query_dataset` RPC via
/// `DataframeQueryTableProvider::with_find_chunks_support(false)`.
///
/// TODO(RR-5817): remove with the SDK's `query_dataset` path.
pub async fn query_dataset_emits_per_segment_pushdown_query_dataset_path(
    service: impl RerunCloudService,
) {
    let service = Arc::new(service);
    for time_type in [
        TimeType::Sequence,
        TimeType::DurationNs,
        TimeType::TimestampNs,
    ] {
        query_dataset_emits_per_segment_pushdown_by_time_type(&service, time_type, false).await;
    }
}

async fn query_dataset_emits_per_segment_pushdown_by_time_type<T: RerunCloudService>(
    service: &Arc<T>,
    time_type: TimeType,
    supports_find_chunks: bool,
) {
    let dataset_name = format!("client_pushdown_{time_type}");
    let tuid_prefix = match time_type {
        TimeType::Sequence => 88,
        TimeType::DurationNs => 89,
        TimeType::TimestampNs => 90,
    };

    // Inline the 3-segment fixture (rather than `register_per_segment_dataset`) so
    // we create the dataset entry exactly once and keep its handle.
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        tuid_prefix,
        [
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "seg1",
                &["my/entity"],
                1000,
                time_type,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "seg2",
                &["my/entity"],
                2000,
                time_type,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "seg3",
                &["my/entity"],
                3000,
                time_type,
            ),
        ],
    );
    let dataset_entry = service.create_dataset_entry_with_name(&dataset_name).await;
    service
        .register_with_dataset_name_blocking(&dataset_name, data_sources_def.to_data_sources())
        .await;
    // Keep the temp RRD files alive: some backends re-read them during `query_dataset`.
    let _data_sources = data_sources_def;

    let segment_ids = ["seg1", "seg2", "seg3"];
    let index_values: BTreeMap<SegmentId, BTreeSet<IndexValue>> = [
        (SegmentId::from("seg1"), 1010),
        (SegmentId::from("seg2"), 2010),
        (SegmentId::from("seg3"), 3010),
    ]
    .into_iter()
    .map(|(seg, value)| (seg, BTreeSet::from([TimeInt::new_temporal(value)])))
    .collect();

    let index_name = re_log_types::build_index_value(0_i64, time_type)
        .0
        .name()
        .as_str();

    let query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some(index_name.into()),
        sparse_fill_strategy: re_chunk_store::SparseFillStrategy::None,
        ..Default::default()
    };

    // Run `scan` against a recording client and return every per-segment value list it emitted
    // (keyed by the segment it was aligned to), the requests it recorded, and the row count the
    // resulting plan actually produced.
    async fn emitted_per_segment_values<T: RerunCloudService>(
        service: &Arc<T>,
        dataset_id: re_log_types::EntryId,
        query: &re_chunk_store::QueryExpression,
        segment_ids: &[&str],
        index_values: &BTreeMap<SegmentId, BTreeSet<IndexValue>>,
        supports_find_chunks: bool,
    ) -> (BTreeMap<SegmentId, Vec<i64>>, Vec<RecordedRequest>, usize) {
        let client = TestClient::new(Arc::clone(service));
        let provider = DataframeQueryTableProvider::new_from_client(
            client.clone(),
            dataset_id,
            query,
            segment_ids,
            Some(Arc::new(index_values.clone())),
            None,
            None,
            Vec::new(),
            Arc::new(NoOpObjectStoreAuthenticator::default()),
        )
        .await
        .expect("build the table provider")
        .with_find_chunks_support(supports_find_chunks);

        let ctx = SessionContext::default();
        // `scan` is where the pushdown runs and the request is sent.
        let plan = provider
            .scan(&ctx.state(), None, &[], None)
            .await
            .expect("plan the scan");

        let captured = client.recorded_requests.lock().clone();

        let num_partitions = plan.output_partitioning().partition_count();
        let results: Vec<_> = (0..num_partitions)
            .map(|partition| plan.execute(partition, ctx.task_ctx()))
            .try_collect()
            .expect("start every partition");
        let batches: Vec<RecordBatch> = futures::stream::iter(results)
            .flat_map(|stream| stream)
            .try_collect()
            .await
            .expect("collect the batches");
        let num_rows: usize = batches.iter().map(RecordBatch::num_rows).sum();

        let mut emitted: BTreeMap<SegmentId, Vec<i64>> = BTreeMap::new();
        for request in &captured {
            match request {
                RecordedRequest::FindChunks(request) => {
                    let ext::ChunkMode::Samples {
                        segments,
                        fill_mode,
                        ..
                    } = &request.mode
                    else {
                        continue;
                    };
                    assert_eq!(
                        *fill_mode,
                        ext::FillMode::None,
                        "a reader without fill must not ask for the latest-at chunks of its \
                         sample times"
                    );
                    for (segment_id, values) in segments {
                        if !values.is_empty() {
                            emitted.insert(
                                segment_id.clone(),
                                values.iter().map(|value| value.as_i64()).collect(),
                            );
                        }
                    }
                }
                RecordedRequest::QueryDataset(request) => {
                    let Some(latest_at) = request
                        .query
                        .as_ref()
                        .and_then(|query| query.latest_at.as_ref())
                    else {
                        continue;
                    };
                    assert_eq!(
                        request.segment_ids.len(),
                        latest_at.per_segment_values.len(),
                        "per_segment_values must carry one list per requested segment"
                    );
                    for (segment_id, values) in
                        std::iter::zip(&request.segment_ids, &latest_at.per_segment_values)
                    {
                        if !values.is_empty() {
                            emitted.insert(segment_id.clone(), values.clone());
                        }
                    }
                }
            }
        }
        (emitted, captured, num_rows)
    }

    // Each segment's sampled value is pushed down, aligned to segment_ids.
    let (pushed, captured, num_rows) = emitted_per_segment_values(
        service,
        dataset_entry.details.id,
        &query,
        &segment_ids,
        &index_values,
        supports_find_chunks,
    )
    .await;
    assert!(!captured.is_empty(), "scan must emit at least one request");
    let expected: BTreeMap<SegmentId, Vec<i64>> = [
        (SegmentId::from("seg1"), vec![1010]),
        (SegmentId::from("seg2"), vec![2010]),
        (SegmentId::from("seg3"), vec![3010]),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        pushed, expected,
        "pushdown must emit per_segment_values aligned to segment_ids"
    );
    assert!(
        num_rows > 0,
        "a populated per-segment selection must return rows"
    );

    // Every segment has an empty value list: the selection selects nothing. The new path skips
    // the RPC entirely, because an all-empty `Samples` map selects nothing; the legacy adapter
    // has no such short-circuit and always issues one request, with `per_segment_values` all
    // empty. Both paths must return no rows.
    let empty_index_values: BTreeMap<SegmentId, BTreeSet<IndexValue>> = segment_ids
        .iter()
        .map(|&segment_id| (SegmentId::from(segment_id), BTreeSet::new()))
        .collect();
    let (empty_pushed, empty_captured, empty_rows) = emitted_per_segment_values(
        service,
        dataset_entry.details.id,
        &query,
        &segment_ids,
        &empty_index_values,
        supports_find_chunks,
    )
    .await;
    assert!(
        empty_pushed.is_empty(),
        "an all-empty per-segment selection must push no values"
    );
    assert_eq!(
        empty_rows, 0,
        "an all-empty per-segment selection must return no rows"
    );
    if supports_find_chunks {
        assert!(
            empty_captured.is_empty(),
            "an all-empty Samples selection must skip the find_chunks RPC entirely"
        );
    } else {
        assert_eq!(
            empty_captured.len(),
            1,
            "the legacy adapter issues exactly one query_dataset request for an all-empty selection"
        );
        let RecordedRequest::QueryDataset(request) = &empty_captured[0] else {
            panic!("expected a QueryDataset request, got a FindChunks request");
        };
        let latest_at = request
            .query
            .as_ref()
            .and_then(|query| query.latest_at.as_ref())
            .expect("the legacy per-segment request must carry latest_at");
        assert!(
            latest_at.per_segment_values.iter().all(Vec::is_empty),
            "legacy per_segment_values must all be empty"
        );
    }
}

/// Wire-level test that the server rejects an invalid combination
/// of `per_segment_values` and `latest_at.at`.
///
/// The ext-level `TryFrom<crate::cloud::v1alpha1::QueryDatasetRequest>` enforces
/// that `at` must be unset (i.e. `STATIC`) when `per_segment_values` is non-empty.
/// Unit tests in [`re_protos`] exercise that conversion directly. This test is
/// the integration-level counterpart: it confirms that a wire request carrying
/// the invalid combination is actually rejected by the server with
/// `InvalidArgument`, on every `RerunCloudService` implementation that goes
/// through the conversion layer. One violation is sufficient — the unit tests
/// already cover the full rule matrix.
pub async fn query_dataset_per_segment_values_validation_rejected(service: impl RerunCloudService) {
    use re_protos::cloud::v1alpha1::QueryDatasetRequest as WireQueryDatasetRequest;
    use re_protos::headers::RerunHeadersInjectorExt as _;

    let dataset_name = "per_segment_validation_rejected";
    // Time-type-independent: the conflict is rejected by the wire `TryFrom`
    // conversion before any data lookup, so one timeline type is sufficient.
    let _data_sources =
        register_per_segment_dataset(&service, dataset_name, 99, TimeType::Sequence).await;

    // Build the *wire* form directly so the server's TryFrom conversion runs
    // — sending the ext form would just fail locally before hitting the
    // server. `at = Some(...)` (i.e. non-STATIC) combined with non-empty
    // `per_segment_values` is the explicit invalid case we're checking.
    let request = WireQueryDatasetRequest {
        segment_ids: vec![
            re_protos::common::v1alpha1::SegmentId {
                id: Some("seg1".to_owned()),
            },
            re_protos::common::v1alpha1::SegmentId {
                id: Some("seg2".to_owned()),
            },
            re_protos::common::v1alpha1::SegmentId {
                id: Some("seg3".to_owned()),
            },
        ],
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::QueryLatestAt {
                index: Some(re_protos::common::v1alpha1::IndexColumnSelector {
                    timeline: Some(re_protos::common::v1alpha1::Timeline {
                        name: "frame_nr".into(),
                    }),
                }),
                // Conflict: `at` set AND per_segment_values set.
                at: Some(1010),
                per_segment_values: vec![
                    re_protos::cloud::v1alpha1::IndexValueList { values: vec![1010] },
                    re_protos::cloud::v1alpha1::IndexValueList { values: vec![2010] },
                    re_protos::cloud::v1alpha1::IndexValueList { values: vec![3010] },
                ],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let result = service
        .query_dataset(
            tonic::Request::new(request)
                .with_entry_name(crate::tests::common::entry_name(dataset_name)),
        )
        .await;

    // `Result::expect_err` would require `Response: Debug`; the trait's
    // associated `QueryDatasetStream` doesn't implement it. Match instead.
    match result {
        Ok(_) => panic!(
            "server must reject `per_segment_values` combined with `at != STATIC` \
             with InvalidArgument; got Ok",
        ),
        Err(err) => {
            assert_eq!(
                err.code(),
                tonic::Code::InvalidArgument,
                "expected InvalidArgument, got {err}",
            );
        }
    }
}

/// RR-4355 wire-level test for the `(select_all_entity_paths=false,
/// entity_paths=[])` truth-table case from `cloud.proto`.
///
/// Per the proto: `(false, [])` is a valid query that yields no results
/// regardless of the rest of the filter. Both the Rerun Hub and OSS
/// `re_server` honor this — the Rerun Hub via the per-segment
/// short-circuit added in this PR, OSS via the entity-filter check in
/// `get_chunks_for_query_results`.
pub async fn query_dataset_per_segment_values_empty_entity_paths_short_circuits(
    service: impl RerunCloudService,
) {
    let service = Arc::new(service);
    for time_type in [
        TimeType::Sequence,
        TimeType::DurationNs,
        TimeType::TimestampNs,
    ] {
        query_dataset_per_segment_values_empty_entity_paths_short_circuits_by_time_type(
            service.as_ref(),
            time_type,
        )
        .await;
    }
}

async fn query_dataset_per_segment_values_empty_entity_paths_short_circuits_by_time_type<
    T: RerunCloudService,
>(
    service: &T,
    time_type: TimeType,
) {
    use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;

    let dataset_name = &format!("per_segment_empty_entity_paths_{time_type}");
    let tuid_prefix = match time_type {
        TimeType::Sequence => 222,
        TimeType::DurationNs => 223,
        TimeType::TimestampNs => 224,
    };
    let _data_sources =
        register_per_segment_dataset(service, dataset_name, tuid_prefix, time_type).await;

    let index_name = re_log_types::build_index_value(0_i64, time_type)
        .0
        .name()
        .as_str();

    let request = QueryDatasetRequest {
        segment_ids: per_segment_segment_ids(),
        // Explicitly: no entity paths, no "all paths". Result must be empty.
        select_all_entity_paths: false,
        entity_paths: vec![],
        query: Some(ext::Query {
            latest_at: Some(ext::QueryLatestAt {
                index: Some(index_name.into()),
                at: TimeInt::STATIC,
                per_segment_values: vec![vec![1010], vec![2010], vec![3010]],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let ids = per_segment_chunk_id_set(service, dataset_name, request).await;

    assert!(
        ids.is_empty(),
        "(select_all_entity_paths=false, entity_paths=[]) must yield an empty \
         result regardless of `per_segment_values`; got {} chunk(s)",
        ids.len(),
    );
}

// ---

/// Options for [`query_dataset_snapshot`].
struct QueryDatasetSnapshotOptions {
    check_schema: bool,
    supports_find_chunks: bool,
}

async fn query_dataset_snapshot<T: RerunCloudService>(
    client: TestClient<T>,
    dataset_entry: &DatasetEntry,
    index_values: Vec<(&str, Vec<i64>)>,
    snapshot_name: &str,
    time_type: TimeType,
    options: QueryDatasetSnapshotOptions,
) {
    let QueryDatasetSnapshotOptions {
        check_schema,
        supports_find_chunks,
    } = options;
    let index_values: BTreeMap<SegmentId, BTreeSet<IndexValue>> = index_values
        .into_iter()
        .map(|(idx, values)| {
            (
                SegmentId::from(idx),
                values.into_iter().map(TimeInt::new_temporal).collect(),
            )
        })
        .collect();

    let timeline_name = match time_type {
        TimeType::Sequence => "frame_nr",
        TimeType::DurationNs => "duration",
        TimeType::TimestampNs => "timestamp",
    };

    let query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some(timeline_name.into()),
        ..Default::default()
    };

    let recorded_requests = Arc::clone(&client.recorded_requests);

    let table_provider = DataframeQueryTableProvider::new_from_client(
        client,
        dataset_entry.details.id,
        &query,
        &[] as &[&str],
        Some(Arc::new(index_values)),
        None,       // arrow_schema — let the provider fetch it
        None,       // trace_headers
        Vec::new(), // metrics_collectors
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .expect("build the table provider")
    .with_find_chunks_support(supports_find_chunks);

    let ctx = SessionContext::default();
    let plan = table_provider
        .scan(&ctx.state(), None, &[], None)
        .await
        .unwrap();
    let schema = plan.schema();

    if !supports_find_chunks {
        let captured = recorded_requests.lock();
        assert!(
            !captured.is_empty(),
            "the query_dataset path must still issue at least one request"
        );
        assert!(
            captured
                .iter()
                .all(|request| matches!(request, RecordedRequest::QueryDataset(_))),
            "the query_dataset path must record only QueryDataset requests"
        );
    }

    let num_partitions = plan.output_partitioning().partition_count();
    let results: Vec<_> = (0..num_partitions)
        .map(|partition| plan.execute(partition, ctx.task_ctx()))
        .try_collect()
        .unwrap();

    let stream = futures::stream::iter(results);

    let results: Vec<RecordBatch> = stream
        .flat_map(|stream| stream)
        .try_collect()
        .await
        .unwrap();

    for batch in &results {
        assert_eq!(batch.schema(), schema);
    }

    let results = if results.is_empty() {
        RecordBatch::new_empty(schema)
    } else {
        concat_record_batches(&results)
    };

    if check_schema {
        insta::assert_snapshot!(
            format!("{snapshot_name}_schema"),
            results.format_schema_snapshot()
        );
    }

    let filtered_results = results.horizontally_sorted().auto_sort_rows().unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_results.format_snapshot(false)
    );
}
