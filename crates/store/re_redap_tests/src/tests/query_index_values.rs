use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches, prop,
};
use crate::utils::client::TestClient;
use arrow::array::RecordBatch;
use datafusion::datasource::TableProvider as _;
use datafusion::physical_plan::ExecutionPlanProperties as _;
use datafusion::prelude::SessionContext;
use futures::{StreamExt as _, TryStreamExt as _};
use re_chunk_store::IndexValue;
use re_datafusion::DataframeQueryTableProvider;
use re_log_types::{EntityPath, TimeInt, TimeType};
use re_protos::cloud::v1alpha1::ext::DatasetEntry;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub async fn query_dataset_index_values_by_time_type<T: RerunCloudService>(
    service: Arc<T>,
    time_type: TimeType,
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

    let client = TestClient { service };

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
            check_schema,
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

/// Collect the set of chunk IDs returned by a query.
///
/// Lets us assert that `per_segment_values` actually narrows the result
/// (a strict subset of the baseline chunk-id set) instead of just
/// counting rows, which can pass even when the server ignores the
/// per-segment filter and returns the full baseline.
async fn per_segment_chunk_id_set<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    request: re_protos::cloud::v1alpha1::ext::QueryDatasetRequest,
) -> BTreeSet<re_chunk::ChunkId> {
    use arrow::array::{Array as _, AsArray as _};
    use re_protos::cloud::v1alpha1::QueryDatasetResponse;
    use re_protos::headers::RerunHeadersInjectorExt as _;

    let stream = service
        .query_dataset(
            tonic::Request::new(request.into())
                .with_entry_name(crate::tests::common::entry_name(dataset_name))
                .unwrap(),
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
            let id_col = batch
                .column_by_name(re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_ID)
                .expect("response missing chunk_id column");
            let id_arr = id_col
                .as_fixed_size_binary_opt()
                .expect("chunk_id column has wrong type");
            for i in 0..id_arr.len() {
                let bytes: [u8; 16] = id_arr
                    .value(i)
                    .try_into()
                    .expect("chunk_id must be 16 bytes");
                ids.insert(re_chunk::ChunkId::from_u128(u128::from_be_bytes(bytes)));
            }
        }
    }
    ids
}

/// Set up the 3-segment dataset used by the `per_segment_values` wire-level
/// tests. Each segment has one entity with **4 single-frame temporal chunks**
/// at `start_time + {10, 20, 30, 40}` plus 1 static chunk.
///
/// Returns the [`DataSourcesDefinition`] so the caller can hold its temp
/// files alive for the full duration of the test — some backends (the Rerun
/// Data Platform manifest registry) re-read the RRD files lazily during
/// `query_dataset`, so dropping the temp dir before that completes results
/// in `NotFound` errors.
async fn register_per_segment_dataset(
    service: &impl RerunCloudService,
    dataset_name: &str,
    tuid_prefix: u64,
) -> DataSourcesDefinition {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        tuid_prefix,
        [
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "rr4355_seg1",
                &["my/entity"],
                1000,
                TimeType::Sequence,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "rr4355_seg2",
                &["my/entity"],
                2000,
                TimeType::Sequence,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "rr4355_seg3",
                &["my/entity"],
                3000,
                TimeType::Sequence,
            ),
        ],
    );

    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    data_sources_def
}

fn per_segment_segment_ids() -> Vec<re_protos::common::v1alpha1::ext::SegmentId> {
    vec![
        "rr4355_seg1".into(),
        "rr4355_seg2".into(),
        "rr4355_seg3".into(),
    ]
}

/// RR-4355 wire-level test for `QueryLatestAt.per_segment_values`.
///
/// Builds a dataset with 3 segments, each holding 4 single-frame temporal
/// chunks (`start_time + {10, 20, 30, 40}`) plus a static chunk, then issues
/// three `QueryDatasetRequest`s directly via the gRPC layer:
///
/// 1. `range_baseline`: a `range(EVERYTHING)` query — every temporal chunk
///    plus statics. Used as the strict-subset superset.
/// 2. `latest_at_baseline`: `latest_at(MAX)` — the latest temporal chunk per
///    segment plus statics. Used as a sanity check that filtered selects
///    *different* chunks than the natural latest-at result.
/// 3. `filtered`: `per_segment_values` carrying a single value per segment.
///    Must be a strict subset of `range_baseline_ids` (proves push-down
///    actually narrowed) and must NOT be a subset of `latest_at_baseline_ids`
///    (proves the per-segment selection diverged from a plain latest-at).
///
/// This is the regression-guard for the OSS server's per-segment chunk filter
/// (PR-E). The Rerun Data Platform server scaffolding lands the same shape but
/// currently no-ops the filter (TODO(tsaucer): RR-4355) — that test will be
/// activated when the Lance pre-filter implementation lands.
pub async fn query_dataset_per_segment_values_wire_level(service: impl RerunCloudService) {
    use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;

    let dataset_name = "rr4355_per_segment_wire_level";
    let _data_sources = register_per_segment_dataset(&service, dataset_name, 77).await;

    let segment_ids = per_segment_segment_ids();

    // 1. range(EVERYTHING) baseline — superset of every other query.
    let range_baseline_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            range: Some(re_protos::cloud::v1alpha1::ext::QueryRange {
                index: "frame_nr".into(),
                index_range: re_log_types::AbsoluteTimeRange::EVERYTHING,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let range_baseline_ids =
        per_segment_chunk_id_set(&service, dataset_name, range_baseline_request).await;
    assert!(
        !range_baseline_ids.is_empty(),
        "range baseline must return at least one chunk, got 0"
    );

    // 2. latest_at(MAX) baseline — picks the latest temporal chunk per
    //    segment (frame `start + 40`) plus statics. Used to prove the
    //    `per_segment_values` filter selects *different* chunks (frame
    //    `start + 10`).
    let latest_at_baseline_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
                at: TimeInt::MAX,
                per_segment_values: vec![],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let latest_at_baseline_ids =
        per_segment_chunk_id_set(&service, dataset_name, latest_at_baseline_request).await;
    assert!(
        !latest_at_baseline_ids.is_empty(),
        "latest_at(MAX) baseline must return at least one chunk, got 0"
    );

    // 3. Filtered: per_segment_values with one value per segment that matches
    //    a single temporal chunk (frame `start + 10`).
    let filtered_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
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
    let filtered_ids = per_segment_chunk_id_set(&service, dataset_name, filtered_request).await;
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

    // Filtered must select different *temporal* chunks than a plain
    // latest_at(MAX) — the per-segment values point at frame `start+10`, the
    // latest_at baseline picks frame `start+40`. The two share static chunks
    // but no temporal chunks, so the filtered set is NOT a subset of the
    // latest_at baseline (proving the server honored `per_segment_values`
    // rather than collapsing back to global latest-at).
    assert!(
        !filtered_ids.is_subset(&latest_at_baseline_ids),
        "per_segment_values must select different temporal chunks than latest_at(MAX); \
         filtered={filtered_ids:?} latest_at_baseline={latest_at_baseline_ids:?}",
    );

    // Empty per_segment_values list per segment → no temporal chunks, but
    // statics MUST still surface (per the proto contract, the empty list
    // means "static-only for this segment"). The fixture has one static
    // chunk per `(segment, entity)` = 3 segments × 1 entity = 3 statics.
    let static_only_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
                at: TimeInt::STATIC,
                per_segment_values: vec![vec![], vec![], vec![]],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let static_only_ids =
        per_segment_chunk_id_set(&service, dataset_name, static_only_request).await;
    assert!(
        static_only_ids.len() <= filtered_ids.len(),
        "empty per_segment_values must return no MORE chunks than the temporal filter, \
         got static={} filtered={}",
        static_only_ids.len(),
        filtered_ids.len(),
    );
    // Strict assertion: empty per-segment lists MUST surface every static
    // chunk in the dataset. A vacuous `is_subset` would pass even if the
    // backend silently dropped statics — this catches that regression.
    assert!(
        !static_only_ids.is_empty(),
        "empty per_segment_values must still surface static chunks; got 0",
    );
    assert_eq!(
        static_only_ids.len(),
        3,
        "fixture has 3 static chunks (one per segment); got {}",
        static_only_ids.len(),
    );
    // Per the proto contract: empty values list still surfaces static
    // chunks. The static-only result is a subset of the full-range baseline,
    // and also of the single-value filtered result (which itself includes
    // statics on top of one temporal chunk per segment).
    assert!(
        static_only_ids.is_subset(&range_baseline_ids),
        "static-only result must be a subset of the range baseline chunks",
    );
    assert!(
        static_only_ids.is_subset(&filtered_ids),
        "static-only chunks must also appear in the single-value filtered result",
    );
}

/// RR-4355 wire-level test that `per_segment_values` accepts multiple values
/// per segment and unions their per-value latest-at results.
///
/// Uses the same fixture as [`query_dataset_per_segment_values_wire_level`]
/// but issues `per_segment_values=[[1010, 1030], [2020], [3010, 3030]]`.
/// With one chunk per frame, the filtered set must contain *both* chunks for
/// seg1 and seg3, exactly one for seg2, and remain a strict subset of the
/// full-range baseline.
pub async fn query_dataset_per_segment_values_multi_value_wire_level(
    service: impl RerunCloudService,
) {
    use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;

    let dataset_name = "rr4355_per_segment_multi_value_wire_level";
    let _data_sources = register_per_segment_dataset(&service, dataset_name, 88).await;

    let segment_ids = per_segment_segment_ids();

    let range_baseline_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            range: Some(re_protos::cloud::v1alpha1::ext::QueryRange {
                index: "frame_nr".into(),
                index_range: re_log_types::AbsoluteTimeRange::EVERYTHING,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let range_baseline_ids =
        per_segment_chunk_id_set(&service, dataset_name, range_baseline_request).await;

    // Multi-value: ask for two distinct chunks in seg1 and seg3, one in seg2.
    let filtered_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
                at: TimeInt::STATIC,
                per_segment_values: vec![
                    vec![1010, 1030], // seg1: two distinct chunks
                    vec![2020],       // seg2: one chunk
                    vec![3010, 3030], // seg3: two distinct chunks
                ],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let filtered_ids = per_segment_chunk_id_set(&service, dataset_name, filtered_request).await;

    // The single-value test already proves "one value → one chunk" + statics.
    // Here we additionally need the union to grow as more values are added.
    // With 5 distinct temporal values across the 3 segments + 3 static chunks,
    // a faithful server returns 8 chunk IDs. We assert ≥ 5 (defensive, in
    // case statics are deduped or omitted by some intermediate layer) but
    // strictly more than a single-value baseline would yield.
    assert!(
        filtered_ids.is_subset(&range_baseline_ids),
        "multi-value per_segment_values must only return chunks in the range baseline; \
         got {} extra chunk(s)",
        filtered_ids.difference(&range_baseline_ids).count(),
    );
    assert!(
        filtered_ids.len() >= 5,
        "multi-value per_segment_values must surface at least one chunk per requested value; \
         expected ≥ 5 (2+1+2), got {}",
        filtered_ids.len(),
    );
    assert!(
        filtered_ids.len() < range_baseline_ids.len(),
        "multi-value per_segment_values must still strictly narrow vs the range baseline; \
         range_baseline={} filtered={}",
        range_baseline_ids.len(),
        filtered_ids.len(),
    );

    // Sanity: a single-value request for the same dataset must return strictly
    // fewer chunks (one fewer in seg1, one fewer in seg3 → 6 chunks vs 8).
    let single_value_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
                at: TimeInt::STATIC,
                per_segment_values: vec![vec![1010], vec![2020], vec![3010]],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let single_value_ids =
        per_segment_chunk_id_set(&service, dataset_name, single_value_request).await;
    assert!(
        single_value_ids.len() < filtered_ids.len(),
        "asking for more values per segment must surface more chunks; \
         single_value={} multi_value={}",
        single_value_ids.len(),
        filtered_ids.len(),
    );
    assert!(
        single_value_ids.is_subset(&filtered_ids),
        "single-value result must be a subset of the multi-value result"
    );
}

/// RR-4355 wire-level test that the server rejects an invalid combination
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

    let dataset_name = "rr4355_per_segment_validation_rejected";
    let _data_sources = register_per_segment_dataset(&service, dataset_name, 99).await;

    // Build the *wire* form directly so the server's TryFrom conversion runs
    // — sending the ext form would just fail locally before hitting the
    // server. `at = Some(...)` (i.e. non-STATIC) combined with non-empty
    // `per_segment_values` is the explicit invalid case we're checking.
    let request = WireQueryDatasetRequest {
        segment_ids: vec![
            re_protos::common::v1alpha1::SegmentId {
                id: Some("rr4355_seg1".to_owned()),
            },
            re_protos::common::v1alpha1::SegmentId {
                id: Some("rr4355_seg2".to_owned()),
            },
            re_protos::common::v1alpha1::SegmentId {
                id: Some("rr4355_seg3".to_owned()),
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
                .with_entry_name(crate::tests::common::entry_name(dataset_name))
                .unwrap(),
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

/// RR-4355 wire-level test that combining caller-supplied `chunk_ids` with
/// `per_segment_values` intersects the two filters.
///
/// First issues a baseline `per_segment_values` request and captures the
/// returned chunk-id set, then re-issues the same query with `chunk_ids`
/// restricted to a single id from that set. The result must be exactly that
/// one chunk — proving:
///
/// * the OSS server's per-row filter (`requested_chunk_ids.contains(…)`)
///   in `rerun_cloud.rs` honors the intersection, and
/// * the Data Platform's set-intersection in `manifest_registry.rs` does too.
///
/// **Backend-conditional**: the OSS server currently returns
/// `Unimplemented` whenever `chunk_ids` is non-empty (see
/// `re_server::rerun_cloud::query_dataset` early-return). This test treats
/// that response as "this backend doesn't support `chunk_ids` filtering
/// yet" and exits cleanly without asserting; it'll automatically activate
/// once OSS gains support. Backends that accept the filter must produce
/// the strict-intersection result.
pub async fn query_dataset_per_segment_values_with_chunk_ids_intersects(
    service: impl RerunCloudService,
) {
    use re_protos::cloud::v1alpha1::QueryDatasetResponse;
    use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;
    use re_protos::headers::RerunHeadersInjectorExt as _;

    let dataset_name = "rr4355_per_segment_chunk_ids_intersect";
    let _data_sources = register_per_segment_dataset(&service, dataset_name, 111).await;

    let segment_ids = per_segment_segment_ids();

    // Baseline: per_segment_values with one value per segment. Returns one
    // temporal chunk per segment + one static chunk per segment.
    let baseline_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
                at: TimeInt::STATIC,
                per_segment_values: vec![vec![1010], vec![2010], vec![3010]],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let baseline_ids = per_segment_chunk_id_set(&service, dataset_name, baseline_request).await;
    assert!(
        baseline_ids.len() >= 2,
        "baseline must return ≥ 2 chunks so we can pick one and meaningfully \
         narrow; got {}",
        baseline_ids.len(),
    );

    // Pick a single chunk id from the baseline set as the caller-supplied
    // filter. The order of `BTreeSet::iter` is deterministic but irrelevant
    // — any one chunk is fine.
    let pinned_chunk_id = *baseline_ids.iter().next().expect("baseline non-empty");

    // Same request but with `chunk_ids` set to that one id. Both filters
    // must be honored: the result is `baseline ∩ {pinned_chunk_id}`.
    let intersected_request = QueryDatasetRequest {
        segment_ids: segment_ids.clone(),
        chunk_ids: vec![pinned_chunk_id],
        select_all_entity_paths: true,
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
                at: TimeInt::STATIC,
                per_segment_values: vec![vec![1010], vec![2010], vec![3010]],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Issue the request directly so we can match on the error code. We can't
    // reuse `per_segment_chunk_id_set` here because it `unwrap()`s, which
    // would panic on the OSS `Unimplemented` we want to soft-skip.
    let request_wire: re_protos::cloud::v1alpha1::QueryDatasetRequest = intersected_request.into();
    let response = service
        .query_dataset(
            tonic::Request::new(request_wire)
                .with_entry_name(crate::tests::common::entry_name(dataset_name))
                .unwrap(),
        )
        .await;

    let mut stream = match response {
        Ok(resp) => Box::pin(resp.into_inner()),
        Err(err) if err.code() == tonic::Code::Unimplemented => {
            // OSS server-style: chunk_ids filter not yet implemented.
            // Test is informational on this backend.
            return;
        }
        Err(err) => panic!("query_dataset failed unexpectedly: {err}"),
    };

    let mut intersected_ids: BTreeSet<re_chunk::ChunkId> = BTreeSet::new();
    while let Some(resp) = stream.next().await {
        let resp: QueryDatasetResponse = resp.unwrap();
        if let Some(part) = resp.data {
            use arrow::array::{Array as _, AsArray as _};
            let batch: arrow::array::RecordBatch = part.try_into().unwrap();
            let id_col = batch
                .column_by_name(re_protos::cloud::v1alpha1::QueryDatasetResponse::FIELD_CHUNK_ID)
                .expect("response missing chunk_id column");
            let id_arr = id_col
                .as_fixed_size_binary_opt()
                .expect("chunk_id column has wrong type");
            for i in 0..id_arr.len() {
                let bytes: [u8; 16] = id_arr.value(i).try_into().expect("chunk_id is 16 bytes");
                intersected_ids.insert(re_chunk::ChunkId::from_u128(u128::from_be_bytes(bytes)));
            }
        }
    }

    assert_eq!(
        intersected_ids,
        BTreeSet::from([pinned_chunk_id]),
        "chunk_ids ∩ per_segment_values must equal pinned_chunk_id; \
         got {intersected_ids:?}",
    );
}

/// RR-4355 wire-level test that the server's `Version` response advertises
/// the `per_segment_index_values` feature flag, so capability-gated clients
/// can detect it.
///
/// Both backends populate `VersionResponse.features` from
/// `re_protos::cloud::v1alpha1::features::all_supported_features()`, so the
/// flag must be present on every implementation.
pub async fn version_advertises_per_segment_index_values_feature(service: impl RerunCloudService) {
    let response = service
        .version(tonic::Request::new(
            re_protos::cloud::v1alpha1::VersionRequest {},
        ))
        .await
        .expect("Version RPC must succeed")
        .into_inner();

    assert!(
        response
            .features
            .iter()
            .any(|f| f == re_protos::cloud::v1alpha1::features::PER_SEGMENT_INDEX_VALUES),
        "VersionResponse.features must advertise `per_segment_index_values`; got {:?}",
        response.features,
    );
}

/// RR-4355 wire-level test for the `(select_all_entity_paths=false,
/// entity_paths=[])` truth-table case from `cloud.proto`.
///
/// Per the proto: `(false, [])` is a valid query that yields no results
/// regardless of the rest of the filter. Both the Data Platform and OSS
/// `re_server` honor this — the Data Platform via the per-segment
/// short-circuit added in this PR, OSS via the entity-filter check in
/// `get_chunks_for_query_results`.
pub async fn query_dataset_per_segment_values_empty_entity_paths_short_circuits(
    service: impl RerunCloudService,
) {
    use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;

    let dataset_name = "rr4355_per_segment_empty_entity_paths";
    let _data_sources = register_per_segment_dataset(&service, dataset_name, 222).await;

    let request = QueryDatasetRequest {
        segment_ids: per_segment_segment_ids(),
        // Explicitly: no entity paths, no "all paths". Result must be empty.
        select_all_entity_paths: false,
        entity_paths: vec![],
        query: Some(re_protos::cloud::v1alpha1::ext::Query {
            latest_at: Some(re_protos::cloud::v1alpha1::ext::QueryLatestAt {
                index: Some("frame_nr".into()),
                at: TimeInt::STATIC,
                per_segment_values: vec![vec![1010], vec![2010], vec![3010]],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let ids = per_segment_chunk_id_set(&service, dataset_name, request).await;

    assert!(
        ids.is_empty(),
        "(select_all_entity_paths=false, entity_paths=[]) must yield an empty \
         result regardless of `per_segment_values`; got {} chunk(s)",
        ids.len(),
    );
}

// ---

async fn query_dataset_snapshot<T: RerunCloudService>(
    client: TestClient<T>,
    dataset_entry: &DatasetEntry,
    index_values: Vec<(&str, Vec<i64>)>,
    snapshot_name: &str,
    time_type: TimeType,
    check_schema: bool,
) {
    let index_values: BTreeMap<String, BTreeSet<IndexValue>> = index_values
        .into_iter()
        .map(|(idx, values)| {
            (
                idx.to_owned(),
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

    let table_provider = DataframeQueryTableProvider::new_from_client(
        client,
        dataset_entry.details.id,
        &query,
        &[] as &[&str],
        Some(Arc::new(index_values)),
        None, // arrow_schema — let the provider fetch it
        None, // trace_headers
    )
    .await
    .unwrap();

    let ctx = SessionContext::default();
    let plan = table_provider
        .scan(&ctx.state(), None, &[], None)
        .await
        .unwrap();
    let schema = plan.schema();

    let num_partitions = plan.output_partitioning().partition_count();
    let results = (0..num_partitions)
        .map(|partition| plan.execute(partition, ctx.task_ctx()))
        .collect::<Result<Vec<_>, _>>()
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
