//! Tests for `ChunkStore::split_by_time_ranges`.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use re_chunk::{Chunk, ChunkId, RowId, TimeInt};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkTrackingMode, IsStartOfGop};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{AbsoluteTimeRange, TimePoint, Timeline};
use re_sdk_types::archetypes::{Pinhole, Transform3D, VideoStream};
use re_sdk_types::components::{
    IsKeyframe, PinholeProjection, TransformFrameId, Translation3D, VideoCodec, VideoSample,
};
use re_types_core::ComponentBatch;

fn frame() -> Timeline {
    Timeline::new_sequence("frame")
}

fn new_store() -> ChunkStore {
    let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "split_test");
    ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED)
}

/// One row: its time on `frame` (`None` means static) and its components.
type Row<'a> = (
    Option<i64>,
    Vec<(re_types_core::ComponentDescriptor, &'a dyn ComponentBatch)>,
);

/// Inserts a single chunk with the given rows.
fn insert_chunk(store: &mut ChunkStore, entity: &str, rows: Vec<Row<'_>>) {
    let mut builder = Chunk::builder(entity);
    for (time, components) in rows {
        let timepoint = time.map_or_else(TimePoint::default, |time| {
            TimePoint::from_iter([(frame(), time)])
        });
        builder = builder.with_component_batches(RowId::new(), timepoint, components);
    }
    store
        .insert_chunk(&Arc::new(builder.build().expect("build chunk")))
        .expect("insert chunk");
}

fn points() -> (
    re_types_core::ComponentDescriptor,
    &'static dyn ComponentBatch,
) {
    static POINTS: [MyPoint; 1] = [MyPoint { x: 1.0, y: 2.0 }];
    (MyPoints::descriptor_points(), &POINTS)
}

fn split(
    store: &ChunkStore,
    range: (i64, i64),
    is_start_of_gop: Option<&IsStartOfGop>,
) -> Vec<(ChunkId, Chunk)> {
    let mut splits = store.split_by_time_ranges(
        ChunkTrackingMode::PanicOnMissing,
        frame().name(),
        &[AbsoluteTimeRange::new(
            TimeInt::new_temporal(range.0),
            TimeInt::new_temporal(range.1),
        )],
        is_start_of_gop,
    );
    assert_eq!(splits.len(), 1);
    splits.remove(0)
}

/// For every `entity:component`, the times of the rows that carry data for it (`None` = static).
fn times_per_component(chunks: &[(ChunkId, Chunk)]) -> BTreeMap<String, Vec<Option<i64>>> {
    let mut result: BTreeMap<String, Vec<Option<i64>>> = BTreeMap::new();
    for (_, chunk) in chunks {
        for component in chunk.components_identifiers() {
            for (time, _row_id) in chunk.iter_component_indices(*frame().name(), component) {
                result
                    .entry(format!("{}:{component}", chunk.entity_path()))
                    .or_default()
                    .push((!time.is_static()).then(|| time.as_i64()));
            }
        }
    }
    for times in result.values_mut() {
        times.sort();
    }
    result
}

/// No two chunks of a split may contain the same row.
fn assert_no_duplicate_rows(chunks: &[(ChunkId, Chunk)]) {
    let mut seen = HashSet::new();
    for (_, chunk) in chunks {
        for row_id in chunk.row_ids() {
            assert!(
                seen.insert(row_id),
                "row {row_id} of {} is duplicated",
                chunk.entity_path()
            );
        }
    }
}

fn some(times: &[i64]) -> Vec<Option<i64>> {
    times.iter().copied().map(Some).collect()
}

#[test]
fn keeps_rows_inside_and_latest_value_before() {
    let mut store = new_store();
    insert_chunk(
        &mut store,
        "/partially_inside",
        [2, 4, 6, 8, 10].map(|t| (Some(t), vec![points()])).into(),
    );
    insert_chunk(
        &mut store,
        "/before",
        [1, 2].map(|t| (Some(t), vec![points()])).into(),
    );
    insert_chunk(
        &mut store,
        "/after",
        [20].map(|t| (Some(t), vec![points()])).into(),
    );

    let chunks = split(&store, (4, 8), None);

    assert_no_duplicate_rows(&chunks);
    assert_eq!(
        times_per_component(&chunks),
        BTreeMap::from([
            // Starts exactly at the range: no extra latest-at row.
            (
                "/partially_inside:example.MyPoints:points".to_owned(),
                some(&[4, 6, 8])
            ),
            // Nothing inside: only the latest value before the range.
            ("/before:example.MyPoints:points".to_owned(), some(&[2])),
        ])
    );
}

#[test]
fn video_starts_on_previous_keyframe() {
    let mut store = new_store();
    let codec = VideoCodec::H264;
    insert_chunk(
        &mut store,
        "/video",
        vec![(None, vec![(VideoStream::descriptor_codec(), &codec as _)])],
    );
    let samples: Vec<VideoSample> = (0..20u8).map(|i| VideoSample::from(vec![i])).collect();
    let is_keyframe: Vec<IsKeyframe> = (0..20).map(|i| IsKeyframe::from(i % 5 == 0)).collect();
    insert_chunk(
        &mut store,
        "/video",
        itertools::izip!(&samples, &is_keyframe, 0..)
            .map(|(sample, is_keyframe, i)| {
                (
                    Some(i),
                    vec![
                        (
                            VideoStream::descriptor_sample(),
                            sample as &dyn ComponentBatch,
                        ),
                        (VideoStream::descriptor_is_keyframe(), is_keyframe as _),
                    ],
                )
            })
            .collect(),
    );

    // Every 5th sample is a keyframe.
    let is_start_of_gop: IsStartOfGop =
        Arc::new(|data, _codec| Ok(data.first().is_some_and(|i| i % 5 == 0)));

    let chunks = split(&store, (7, 12), Some(&is_start_of_gop));
    assert_no_duplicate_rows(&chunks);
    let times = times_per_component(&chunks);
    assert_eq!(
        times["/video:VideoStream:sample"],
        some(&[5, 6, 7, 8, 9, 10, 11, 12])
    );
    // The keyframe markers go along with the samples.
    assert_eq!(
        times["/video:VideoStream:is_keyframe"],
        some(&[5, 6, 7, 8, 9, 10, 11, 12])
    );
    assert_eq!(times["/video:VideoStream:codec"], vec![None]);

    // Without keyframe detection, the stream starts at the range.
    let chunks = split(&store, (7, 12), None);
    let times = times_per_component(&chunks);
    assert_eq!(
        times["/video:VideoStream:sample"],
        some(&[7, 8, 9, 10, 11, 12])
    );
}

#[test]
fn keeps_all_multiplexed_transforms_before_range() {
    let mut store = new_store();
    let child = TransformFrameId::new("child");
    let translation = Translation3D::new(1.0, 2.0, 3.0);
    insert_chunk(
        &mut store,
        "/tf",
        [1, 2, 3, 10]
            .map(|t| {
                (
                    Some(t),
                    vec![
                        (
                            Transform3D::descriptor_child_frame(),
                            &child as &dyn ComponentBatch,
                        ),
                        (Transform3D::descriptor_translation(), &translation as _),
                    ],
                )
            })
            .into(),
    );

    let chunks = split(&store, (5, 12), None);

    assert_no_duplicate_rows(&chunks);
    let times = times_per_component(&chunks);
    assert_eq!(times["/tf:Transform3D:child_frame"], some(&[1, 2, 3, 10]));
    assert_eq!(times["/tf:Transform3D:translation"], some(&[1, 2, 3, 10]));
}

#[test]
fn keeps_latest_transform_and_pinhole_before_range() {
    let mut store = new_store();
    let translation = Translation3D::new(1.0, 2.0, 3.0);
    let projection = PinholeProjection::from([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    insert_chunk(
        &mut store,
        "/tf",
        [0, 1]
            .map(|t| {
                (
                    Some(t),
                    vec![(
                        Transform3D::descriptor_translation(),
                        &translation as &dyn ComponentBatch,
                    )],
                )
            })
            .into(),
    );
    insert_chunk(
        &mut store,
        "/camera",
        vec![(
            Some(0),
            vec![(Pinhole::descriptor_image_from_camera(), &projection as _)],
        )],
    );

    let chunks = split(&store, (5, 8), None);

    assert_no_duplicate_rows(&chunks);
    let times = times_per_component(&chunks);
    // Without multiplexed coordinate frames, these are regular latest-at data.
    assert_eq!(times["/tf:Transform3D:translation"], some(&[1]));
    assert_eq!(times["/camera:Pinhole:image_from_camera"], some(&[0]));
}

#[test]
fn keeps_static_data() {
    let mut store = new_store();
    let projection = PinholeProjection::from([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let translation = Translation3D::new(1.0, 2.0, 3.0);
    insert_chunk(
        &mut store,
        "/camera",
        vec![(
            None,
            vec![(Pinhole::descriptor_image_from_camera(), &projection as _)],
        )],
    );
    insert_chunk(
        &mut store,
        "/static_tf",
        vec![(
            None,
            vec![(Transform3D::descriptor_translation(), &translation as _)],
        )],
    );
    insert_chunk(&mut store, "/static_points", vec![(None, vec![points()])]);
    insert_chunk(
        &mut store,
        "/points",
        [5].map(|t| (Some(t), vec![points()])).into(),
    );

    let chunks = split(&store, (4, 8), None);

    assert_no_duplicate_rows(&chunks);
    assert_eq!(
        times_per_component(&chunks),
        BTreeMap::from([
            ("/camera:Pinhole:image_from_camera".to_owned(), vec![None]),
            ("/points:example.MyPoints:points".to_owned(), some(&[5])),
            (
                "/static_points:example.MyPoints:points".to_owned(),
                vec![None]
            ),
            ("/static_tf:Transform3D:translation".to_owned(), vec![None]),
        ])
    );
}

/// A chunk can hold both special (pinhole, transform, video) and regular components, e.g. after compaction.
#[test]
fn mixed_chunks_are_not_duplicated() {
    let mut store = new_store();
    let projection = PinholeProjection::from([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    insert_chunk(
        &mut store,
        "/camera",
        (1..=10)
            .map(|t| {
                (
                    Some(t),
                    vec![
                        (Pinhole::descriptor_image_from_camera(), &projection as _),
                        points(),
                    ],
                )
            })
            .collect(),
    );

    let child = TransformFrameId::new("child");
    insert_chunk(
        &mut store,
        "/tf",
        (1..=10)
            .map(|t| {
                (
                    Some(t),
                    vec![
                        (
                            Transform3D::descriptor_child_frame(),
                            &child as &dyn ComponentBatch,
                        ),
                        points(),
                    ],
                )
            })
            .collect(),
    );

    let chunks = split(&store, (4, 6), None);

    assert_no_duplicate_rows(&chunks);
    let times = times_per_component(&chunks);
    assert_eq!(times["/camera:Pinhole:image_from_camera"], some(&[4, 5, 6]));
    assert_eq!(times["/camera:example.MyPoints:points"], some(&[4, 5, 6]));
    // All transforms before the range are kept, but not the other data logged next to them.
    assert_eq!(
        times["/tf:Transform3D:child_frame"],
        some(&[1, 2, 3, 4, 5, 6])
    );
    assert_eq!(times["/tf:example.MyPoints:points"], some(&[4, 5, 6]));
}
