//! `FindChunks` tests for the redap test suite, run against the OSS server and the Rerun Hub.
//!
//! Each test registers a small dataset, then checks the exact chunk ids `FindChunks` returns.

use std::collections::{BTreeMap, BTreeSet};

use arrow::array::RecordBatch;
use futures::StreamExt as _;
use itertools::izip;
use re_log_types::example_components::MyPoints;
use re_log_types::{EntityPath, TimeInt, TimeType};
use re_protos::cloud::v1alpha1::ext::{
    ChunkCandidatesDataframe, ChunkMode, ComponentSelection, ContentSelection, DirectUrlMode,
    EntitySelection, FillMode, FindChunksRequest, RangeSelection, SegmentSelection,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_types_core::{ChunkId, LayerName, SegmentId};

use crate::tests::common::{RerunCloudServiceExt as _, concat_record_batches, entry_name};
use crate::utils::rerun::next_chunk_id_generator;
use crate::utils::{
    register_layered_dataset, register_mixed_timelines_dataset, register_per_segment_dataset,
    register_simple_dataset,
};

// --- Request builders ---

fn static_request(segments: SegmentSelection) -> FindChunksRequest {
    FindChunksRequest {
        contents: ContentSelection::All,
        mode: ChunkMode::Static { segments },
        direct_url_mode: DirectUrlMode::Signed,
    }
}

fn range_request(
    timeline: &'static str,
    start_inclusive: Option<TimeInt>,
    end_inclusive: Option<TimeInt>,
    fill_mode: FillMode,
    segments: SegmentSelection,
) -> FindChunksRequest {
    FindChunksRequest {
        contents: ContentSelection::All,
        mode: ChunkMode::Range {
            segments,
            range: RangeSelection {
                timeline: timeline.into(),
                start_inclusive,
                end_inclusive,
                fill_mode,
            },
        },
        direct_url_mode: DirectUrlMode::Signed,
    }
}

fn samples_request(
    timeline: &'static str,
    segments: BTreeMap<SegmentId, BTreeSet<TimeInt>>,
    fill_mode: FillMode,
) -> FindChunksRequest {
    FindChunksRequest {
        contents: ContentSelection::All,
        mode: ChunkMode::Samples {
            timeline: timeline.into(),
            segments,
            fill_mode,
        },
        direct_url_mode: DirectUrlMode::Signed,
    }
}

// --- Wire round-trip helpers ---

async fn find_chunks_batches_wire<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    wire: re_protos::cloud::v1alpha1::FindChunksRequest,
) -> Vec<RecordBatch> {
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

async fn find_chunks_batches<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    request: FindChunksRequest,
) -> Vec<RecordBatch> {
    find_chunks_batches_wire(service, dataset_name, request.into()).await
}

async fn find_chunks_error<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    request: FindChunksRequest,
) -> tonic::Status {
    let wire: re_protos::cloud::v1alpha1::FindChunksRequest = request.into();
    find_chunks_wire_error(service, dataset_name, wire).await
}

async fn find_chunks_wire_error<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    wire: re_protos::cloud::v1alpha1::FindChunksRequest,
) -> tonic::Status {
    match service
        .find_chunks(tonic::Request::new(wire).with_entry_name(entry_name(dataset_name)))
        .await
    {
        Ok(_) => panic!("expected find_chunks to fail with an error, got Ok"),
        Err(err) => err,
    }
}

fn chunk_ids(batches: &[RecordBatch]) -> BTreeSet<ChunkId> {
    let mut ids = BTreeSet::new();
    for batch in batches {
        let column = ChunkCandidatesDataframe::COLUMN_CHUNK_ID
            .extract(batch)
            .expect("bad chunk_id column in FindChunks response");
        ids.extend(column.iter_owned());
    }
    ids
}

async fn find_chunks_ids<T: RerunCloudService>(
    service: &T,
    dataset_name: &str,
    request: FindChunksRequest,
) -> BTreeSet<ChunkId> {
    chunk_ids(&find_chunks_batches(service, dataset_name, request).await)
}

/// Asserts that `batches` is exactly one batch with no rows. `FindChunks` answers every empty
/// selection this way, so the client still receives the response schema.
fn assert_one_empty_batch(batches: &[RecordBatch], context: &str) {
    assert_eq!(
        batches.len(),
        1,
        "{context}: an empty selection must return exactly one batch"
    );
    assert_eq!(
        batches[0].num_rows(),
        0,
        "{context}: an empty selection's one batch must have zero rows"
    );
}

// --- Expected chunk ids ---
//
// The test recordings (`create_simple_recording` and `create_simple_recording_one_chunk_per_frame`
// in `utils/rerun.rs`) number their chunks in a fixed order, starting from a per-recording
// `tuid_prefix`. These helpers recompute the same ids.

/// Chunk ids `create_simple_recording` assigns for `entity_count` entities, in entity order:
/// one `(temporal, static)` pair per entity.
fn simple_recording_chunk_ids(tuid_prefix: u64, entity_count: usize) -> Vec<(ChunkId, ChunkId)> {
    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    (0..entity_count)
        .map(|_| (next_chunk_id(), next_chunk_id()))
        .collect()
}

/// Chunk ids `create_simple_recording_one_chunk_per_frame` assigns for its one entity: four
/// temporal chunks (frame offsets 10, 20, 30, 40, in that order) plus one static chunk.
fn one_chunk_per_frame_ids(tuid_prefix: u64) -> ([ChunkId; 4], ChunkId) {
    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let temporal = [
        next_chunk_id(),
        next_chunk_id(),
        next_chunk_id(),
        next_chunk_id(),
    ];
    (temporal, next_chunk_id())
}

// --- Tests ---

/// Static, unbounded Range, and range bounds at the edges of a multi-frame chunk.
///
/// The dataset (`register_simple_dataset`) has three segments, with `tuid_prefix` 1, 2, and 3.
/// Each entity has one temporal chunk spanning frames 10 to 40, and one static chunk.
pub async fn find_chunks_simple_dataset(service: impl RerunCloudService) {
    let dataset_name = "dataset";
    let _data_sources_def = register_simple_dataset(&service, dataset_name).await;

    let seg1 = simple_recording_chunk_ids(1, 2); // my/entity, my/other/entity
    let seg2 = simple_recording_chunk_ids(2, 1); // my/entity
    let seg3 = simple_recording_chunk_ids(3, 3); // my/entity, another/one, yet/another/one

    let all_chunks = itertools::chain!(&seg1, &seg2, &seg3);
    let all_temporal: BTreeSet<ChunkId> = all_chunks.clone().map(|(t, _)| *t).collect();
    let all_static: BTreeSet<ChunkId> = all_chunks.map(|(_, s)| *s).collect();
    let temporal_and_static: BTreeSet<ChunkId> = all_temporal.union(&all_static).copied().collect();

    let static_ids = find_chunks_ids(
        &service,
        dataset_name,
        static_request(SegmentSelection::All),
    )
    .await;
    assert_eq!(
        static_ids, all_static,
        "Static must return exactly the static chunks"
    );

    let unbounded_ids = find_chunks_ids(
        &service,
        dataset_name,
        range_request(
            "frame_nr",
            None,
            None,
            FillMode::None,
            SegmentSelection::All,
        ),
    )
    .await;
    assert_eq!(
        unbounded_ids, temporal_and_static,
        "unbounded Range must return every temporal and static chunk"
    );

    // A range selects a chunk when the two overlap. Each temporal chunk spans frames 10 to 40, so
    // a range that starts at 40 or ends at 10 includes it, and one that starts at 41 or ends at 9
    // does not.
    for (name, start, end) in [
        ("from_span_end", Some(TimeInt::new_temporal(40)), None),
        ("to_span_start", None, Some(TimeInt::new_temporal(10))),
    ] {
        let ids = find_chunks_ids(
            &service,
            dataset_name,
            range_request(
                "frame_nr",
                start,
                end,
                FillMode::None,
                SegmentSelection::All,
            ),
        )
        .await;
        assert_eq!(
            ids, temporal_and_static,
            "{name}: a range touching the temporal span must include it"
        );
    }
    for (name, start, end) in [
        ("from_past_span_end", Some(TimeInt::new_temporal(41)), None),
        ("to_before_span_start", None, Some(TimeInt::new_temporal(9))),
    ] {
        let ids = find_chunks_ids(
            &service,
            dataset_name,
            range_request(
                "frame_nr",
                start,
                end,
                FillMode::None,
                SegmentSelection::All,
            ),
        )
        .await;
        assert_eq!(
            ids, all_static,
            "{name}: a range outside the temporal span must return only statics"
        );
    }

    // Range with fill, at a populated time within the span.
    let fill_ids = find_chunks_ids(
        &service,
        dataset_name,
        range_request(
            "frame_nr",
            Some(TimeInt::new_temporal(25)),
            Some(TimeInt::new_temporal(25)),
            FillMode::LatestAt,
            SegmentSelection::All,
        ),
    )
    .await;
    assert_eq!(
        fill_ids, temporal_and_static,
        "Range with fill at a populated time must resolve every entity's covering chunk, \
         statics included"
    );

    // Range with fill selecting only `labels`, which every entity logs statically and never on a
    // timeline, must return each segment's `my/entity` static chunk.
    let labels_only = FindChunksRequest {
        contents: ContentSelection::Explicit(vec![EntitySelection {
            entity_path: EntityPath::from("my/entity"),
            components: ComponentSelection::Explicit(BTreeSet::from([
                MyPoints::descriptor_labels().component.to_string(),
            ])),
        }]),
        ..range_request(
            "frame_nr",
            Some(TimeInt::new_temporal(25)),
            Some(TimeInt::new_temporal(25)),
            FillMode::LatestAt,
            SegmentSelection::All,
        )
    };
    let labels_ids = find_chunks_ids(&service, dataset_name, labels_only).await;
    let [(t1, s1), (t2, s2), (t3, s3)] = [seg1[0], seg2[0], seg3[0]];
    let my_entity_static = BTreeSet::from([s1, s2, s3]);
    let my_entity_all = BTreeSet::from([t1, s1, t2, s2, t3, s3]);
    // The check accepts a range on purpose: the OSS server narrows by entity only, so it may also
    // return `my/entity`'s temporal chunks, while the Hub returns only the statics.
    assert!(
        labels_ids.is_superset(&my_entity_static) && labels_ids.is_subset(&my_entity_all),
        "Range with fill selecting only a static-only component must return its static chunks, \
         and nothing outside its entity, got {labels_ids:?}"
    );
}

/// Range bounds: equal bounds, the smallest valid time, and a shared endpoint.
///
/// The dataset (`register_per_segment_dataset`) has one entity per segment. It has a
/// single-frame temporal chunk at 10, 20, 30, and 40 frames past the segment's start (1000 for
/// `seg1`), and one static chunk.
pub async fn find_chunks_range_bounds(service: impl RerunCloudService) {
    let dataset_name = "range_bounds_dataset";
    let tuid_prefix = 500;
    let _data_sources =
        register_per_segment_dataset(&service, dataset_name, tuid_prefix, TimeType::Sequence).await;

    let (t, s) = one_chunk_per_frame_ids(tuid_prefix);
    let seg1 = SegmentSelection::Explicit(BTreeSet::from([SegmentId::from("seg1")]));

    // (case name, start_inclusive, end_inclusive, expected chunk ids).
    type RangeCase = (
        &'static str,
        Option<TimeInt>,
        Option<TimeInt>,
        BTreeSet<ChunkId>,
    );

    let cases: Vec<RangeCase> = vec![
        (
            "equal_bound_populated",
            Some(TimeInt::new_temporal(1010)),
            Some(TimeInt::new_temporal(1010)),
            BTreeSet::from([t[0], s]),
        ),
        (
            "equal_bound_unpopulated",
            Some(TimeInt::new_temporal(1015)),
            Some(TimeInt::new_temporal(1015)),
            BTreeSet::from([s]),
        ),
        (
            "upper_only_min",
            None,
            Some(TimeInt::MIN),
            BTreeSet::from([s]),
        ),
        (
            "shared_endpoint_a",
            Some(TimeInt::new_temporal(1010)),
            Some(TimeInt::new_temporal(1020)),
            BTreeSet::from([t[0], t[1], s]),
        ),
    ];

    for (name, start, end, expected) in cases {
        let ids = find_chunks_ids(
            &service,
            dataset_name,
            range_request("frame_nr", start, end, FillMode::None, seg1.clone()),
        )
        .await;
        assert_eq!(ids, expected, "case `{name}`");
    }

    // Fill only on `seg1`: the Hub aligns latest-at once across all selected segments, so a fill
    // over several segments would test that limit instead of the bounds.
    // TODO(RR-5866): fill over every segment once the Hub resolves fill per segment.
    let fill_cases: Vec<(&str, Option<TimeInt>, Option<TimeInt>, ChunkId)> = vec![
        (
            "equal_bound_populated_fill",
            Some(TimeInt::new_temporal(1010)),
            Some(TimeInt::new_temporal(1010)),
            t[0],
        ),
        (
            "equal_bound_unpopulated_fill",
            Some(TimeInt::new_temporal(1015)),
            Some(TimeInt::new_temporal(1015)),
            t[0],
        ),
        ("max_max_fill", Some(TimeInt::MAX), Some(TimeInt::MAX), t[3]),
    ];

    for (name, start, end, expected_temporal) in fill_cases {
        let ids = find_chunks_ids(
            &service,
            dataset_name,
            range_request("frame_nr", start, end, FillMode::LatestAt, seg1.clone()),
        )
        .await;
        assert_eq!(
            ids,
            BTreeSet::from([expected_temporal, s]),
            "case `{name}`: filling must resolve exactly the one covering chunk plus the static \
             chunk"
        );
    }
}

/// Samples: per segment, the chunk that holds the latest value at each sample time with latest-at,
/// or only a chunk with data at that time without. Static chunks either way.
///
/// The dataset is the one `find_chunks_range_bounds` describes.
pub async fn find_chunks_samples(service: impl RerunCloudService) {
    let dataset_name = "samples_dataset";
    let tuid_prefix = 600;
    let _data_sources =
        register_per_segment_dataset(&service, dataset_name, tuid_prefix, TimeType::Sequence).await;

    let (t1, s1) = one_chunk_per_frame_ids(tuid_prefix);
    let (t2, s2) = one_chunk_per_frame_ids(tuid_prefix + 1);
    let (t3, s3) = one_chunk_per_frame_ids(tuid_prefix + 2);

    // Frames 1010 and 3010 hold a chunk. Frame 2015 falls between seg2's chunks at 2010 and 2020,
    // so latest-at picks the one at 2010.
    let samples = BTreeMap::from([
        (
            SegmentId::from("seg1"),
            BTreeSet::from([TimeInt::new_temporal(1010)]),
        ),
        (
            SegmentId::from("seg2"),
            BTreeSet::from([TimeInt::new_temporal(2015)]),
        ),
        (
            SegmentId::from("seg3"),
            BTreeSet::from([TimeInt::new_temporal(3010)]),
        ),
    ]);

    let ids = find_chunks_ids(
        &service,
        dataset_name,
        samples_request("frame_nr", samples.clone(), FillMode::LatestAt),
    )
    .await;
    assert_eq!(
        ids,
        BTreeSet::from([t1[0], t2[0], t3[0], s1, s2, s3]),
        "Samples with latest-at must select each segment's latest chunk at its sample time, plus \
         statics"
    );

    let ids = find_chunks_ids(
        &service,
        dataset_name,
        samples_request("frame_nr", samples, FillMode::None),
    )
    .await;
    assert_eq!(
        ids,
        BTreeSet::from([t1[0], t3[0], s1, s2, s3]),
        "Samples without latest-at must select only the chunks with data at the sample times, \
         plus statics"
    );
}

/// Layers: each (segment, layer) pair returns exactly its own chunks.
///
/// The dataset (`register_layered_dataset`) has three segments, and two of them have a second
/// layer. Each layer is one recording of one entity, with `tuid_prefix` 1 to 5 in registration
/// order.
pub async fn find_chunks_with_layers(service: impl RerunCloudService) {
    let dataset_name = "dataset_with_layers";
    let _data_sources_def = register_layered_dataset(&service, dataset_name).await;

    // (temporal, static) chunk ids, per layer.
    let one_entity = |tuid_prefix: u64| -> BTreeSet<ChunkId> {
        let pair: [ChunkId; 2] = simple_recording_chunk_ids(tuid_prefix, 1)[0].into();
        pair.into_iter().collect()
    };

    let expected: BTreeMap<(SegmentId, LayerName), BTreeSet<ChunkId>> = [
        (
            (SegmentId::from("partition1"), LayerName::base()),
            one_entity(1),
        ),
        (
            (
                SegmentId::from("partition1"),
                LayerName::try_new("extra").expect("valid layer name"),
            ),
            one_entity(2),
        ),
        (
            (
                SegmentId::from("partition2"),
                LayerName::try_new("base").expect("valid layer name"),
            ),
            one_entity(3),
        ),
        (
            (
                SegmentId::from("partition2"),
                LayerName::try_new("extra").expect("valid layer name"),
            ),
            one_entity(4),
        ),
        (
            (SegmentId::from("partition3"), LayerName::base()),
            one_entity(5),
        ),
    ]
    .into_iter()
    .collect();

    let batches = find_chunks_batches(
        &service,
        dataset_name,
        range_request(
            "frame_nr",
            None,
            None,
            FillMode::None,
            SegmentSelection::All,
        ),
    )
    .await;

    let mut actual: BTreeMap<(SegmentId, LayerName), BTreeSet<ChunkId>> = BTreeMap::new();
    for batch in &batches {
        let ids = ChunkCandidatesDataframe::COLUMN_CHUNK_ID
            .extract(batch)
            .expect("bad chunk_id column");
        let segments = ChunkCandidatesDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(batch)
            .expect("bad chunk_segment_id column");
        let layers = ChunkCandidatesDataframe::COLUMN_RERUN_SEGMENT_LAYER
            .extract(batch)
            .expect("bad rerun_segment_layer column");

        for (id, segment, layer) in
            izip!(ids.iter_owned(), segments.iter_owned(), layers.iter_owned())
        {
            actual.entry((segment, layer)).or_default().insert(id);
        }
    }

    assert_eq!(
        actual, expected,
        "membership per (segment, layer) must survive FindChunks"
    );
}

/// The fixed response schema, and `chunk_timeline_start` when only some chunks have the request's
/// timeline.
///
/// The dataset (`register_mixed_timelines_dataset`) has two segments of one entity, with
/// `tuid_prefix` 1 and 2: `segment_sequence` on `frame_nr`, and `segment_timestamp` on
/// `timestamp`.
pub async fn find_chunks_fixed_schema(service: impl RerunCloudService) {
    let dataset_name = "dataset_mixed_timelines";
    let _data_sources_def = register_mixed_timelines_dataset(&service, dataset_name).await;

    // The request uses `frame_nr`, which `segment_timestamp` does not have.
    let (t_seq, s_seq) = simple_recording_chunk_ids(1, 1)[0];
    let (t_foreign, s_foreign) = simple_recording_chunk_ids(2, 1)[0];

    let batches = find_chunks_batches(
        &service,
        dataset_name,
        range_request(
            "frame_nr",
            None,
            None,
            FillMode::None,
            SegmentSelection::All,
        ),
    )
    .await;
    assert!(!batches.is_empty(), "expected at least one response batch");

    for batch in &batches {
        assert_eq!(
            batch.schema().as_ref(),
            &ChunkCandidatesDataframe::max_schema(),
            "every FindChunks batch must have the fixed max schema",
        );
    }

    let merged = concat_record_batches(&batches);
    let ids = ChunkCandidatesDataframe::COLUMN_CHUNK_ID
        .extract(&merged)
        .expect("bad chunk_id column");
    let is_static = ChunkCandidatesDataframe::COLUMN_CHUNK_IS_STATIC
        .extract(&merged)
        .expect("bad chunk_is_static column");
    let starts = ChunkCandidatesDataframe::COLUMN_CHUNK_TIMELINE_START
        .extract(&merged)
        .expect("bad chunk_timeline_start column");

    let mut by_id: BTreeMap<ChunkId, (bool, Option<i64>)> = BTreeMap::new();
    for (id, is_static, start) in izip!(ids.iter_owned(), is_static.iter_owned(), starts.iter()) {
        by_id.insert(id, (is_static, start));
    }

    assert_eq!(
        by_id.get(&t_seq),
        Some(&(false, Some(10))),
        "a temporal chunk's start must be its minimum on the request timeline"
    );
    assert_eq!(
        by_id.get(&s_seq),
        Some(&(true, None)),
        "static chunks must have a null timeline start"
    );

    // An unbounded Range selects every chunk, whatever its timelines, so `segment_timestamp`'s
    // chunks are candidates too, with a null start because they have no `frame_nr` values.
    assert_eq!(
        by_id.get(&t_foreign),
        Some(&(false, None)),
        "a temporal chunk without the request timeline must be a candidate with a null start"
    );
    assert_eq!(
        by_id.get(&s_foreign),
        Some(&(true, None)),
        "static chunks must have a null timeline start"
    );
}

/// Segment scope: every segment, one explicit segment, and the two kinds of empty scope.
///
/// The dataset is the one `find_chunks_range_bounds` describes.
pub async fn find_chunks_segment_scope(service: impl RerunCloudService) {
    let dataset_name = "segment_scope_dataset";
    let tuid_prefix = 800;
    let _data_sources =
        register_per_segment_dataset(&service, dataset_name, tuid_prefix, TimeType::Sequence).await;

    let all_segment_ids: BTreeSet<SegmentId> = ["seg1", "seg2", "seg3"]
        .into_iter()
        .map(SegmentId::from)
        .collect();

    let static_all = find_chunks_ids(
        &service,
        dataset_name,
        static_request(SegmentSelection::All),
    )
    .await;
    let static_explicit_all = find_chunks_ids(
        &service,
        dataset_name,
        static_request(SegmentSelection::Explicit(all_segment_ids)),
    )
    .await;
    assert_eq!(
        static_all, static_explicit_all,
        "All and Explicit(every segment) must select the same chunks"
    );

    let (_, seg1_static) = one_chunk_per_frame_ids(tuid_prefix);
    let seg1_only = find_chunks_ids(
        &service,
        dataset_name,
        static_request(SegmentSelection::Explicit(BTreeSet::from([
            SegmentId::from("seg1"),
        ]))),
    )
    .await;
    assert_eq!(
        seg1_only,
        BTreeSet::from([seg1_static]),
        "Explicit(seg1) must select only seg1's chunks"
    );

    // Both select nothing, by different routes: the scope lists no segment, or it lists only a
    // segment that is not registered.
    for (name, segments) in [
        ("Explicit(empty)", BTreeSet::new()),
        (
            "Explicit(unknown id)",
            BTreeSet::from([SegmentId::from("doesnt_exist")]),
        ),
    ] {
        let batches = find_chunks_batches(
            &service,
            dataset_name,
            static_request(SegmentSelection::Explicit(segments)),
        )
        .await;
        assert_one_empty_batch(&batches, name);
    }
}

/// The errors each server reports itself, and requests on a dataset without data.
///
/// Request validation that both servers share is tested in `re_protos`.
pub async fn find_chunks_invalid_requests(service: impl RerunCloudService) {
    let dataset_name = "dataset";
    let _data_sources_def = register_simple_dataset(&service, dataset_name).await;

    let empty_dataset_name = "empty_dataset";
    service
        .create_dataset_entry_with_name(empty_dataset_name)
        .await;

    // An unsupported fill mode fails before any shortcut for an empty selection or a dataset
    // without data.
    for (name, dataset, segments) in [
        ("all segments", dataset_name, SegmentSelection::All),
        (
            "no segments",
            dataset_name,
            SegmentSelection::Explicit(BTreeSet::new()),
        ),
        (
            "dataset without data",
            empty_dataset_name,
            SegmentSelection::All,
        ),
    ] {
        let earliest_at = range_request(
            "frame_nr",
            Some(TimeInt::new_temporal(0)),
            Some(TimeInt::new_temporal(10)),
            FillMode::EarliestAt,
            segments,
        );
        let err = find_chunks_error(&service, dataset, earliest_at).await;
        assert_eq!(err.code(), tonic::Code::Unimplemented, "{name}: got {err}");
    }

    let unknown_timeline = range_request(
        "no_such_timeline",
        Some(TimeInt::new_temporal(0)),
        Some(TimeInt::new_temporal(10)),
        FillMode::None,
        SegmentSelection::All,
    );
    let err = find_chunks_error(&service, dataset_name, unknown_timeline).await;
    assert_eq!(err.code(), tonic::Code::InvalidArgument, "got {err}");

    // A dataset without data has no timelines to check against, so every mode answers one empty
    // batch.
    for (name, request) in [
        ("Static", static_request(SegmentSelection::All)),
        (
            "Range",
            range_request(
                "frame_nr",
                None,
                None,
                FillMode::None,
                SegmentSelection::All,
            ),
        ),
    ] {
        let batches = find_chunks_batches(&service, empty_dataset_name, request).await;
        assert_one_empty_batch(&batches, &format!("{name} on an empty dataset"));
    }
}
