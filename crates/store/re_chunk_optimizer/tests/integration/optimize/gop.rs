//! GoP batching: keyframe-aligned video chunks and the regenerated `is_keyframe` column.
//!
//! The fixtures are VP8: its detector never fails and only reads a ten-byte header.

use std::collections::BTreeSet;
use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, UInt32Array};
use arrow::datatypes::DataType;
use futures::StreamExt as _;
use futures::executor::block_on;

use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, RowId, TimelineName};
use re_chunk_optimizer::{
    Error, GopBatchingRule, KeyframePlacement, MergeSplitOverride, OptimizationSettings,
    VideoStreamErrorSource, optimize,
};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{TimePoint, Timeline};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::{IsKeyframe, VideoCodec, VideoSample};
use re_types_core::{ArrowDataType as _, ComponentBatch as _};

use super::helpers::*;

const SAMPLE_LEN: usize = 1024;

// --- Fixtures ---

fn frame() -> Timeline {
    Timeline::new_sequence("frame")
}

fn frame_name() -> TimelineName {
    *frame().name()
}

fn sample_column() -> ComponentIdentifier {
    VideoStream::descriptor_sample().component
}

fn keyframe_column() -> ComponentIdentifier {
    VideoStream::descriptor_is_keyframe().component
}

fn codec_column() -> ComponentIdentifier {
    VideoStream::descriptor_codec().component
}

/// Row `i` (1-based) of the chunk with id `chunk`, as the fixtures number them.
fn row(chunk: u128, i: u128) -> RowId {
    RowId::from_u128((chunk << 32) + i)
}

/// A VP8 keyframe or interframe of `len` bytes.
fn vp8_sample(keyframe: bool, len: usize) -> Vec<u8> {
    let mut data = vec![0_u8; len];
    if keyframe {
        data[3..6].copy_from_slice(&[0x9d, 0x01, 0x2a]);
        data[6..10].copy_from_slice(&[16, 0, 16, 0]);
    } else {
        data[0] = 0x01;
    }
    data
}

/// An H.264 Annex B keyframe: an SPS NAL unit, then an IDR one.
const H264_KEYFRAME: [u8; 57] = [
    0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84, 0x00, 0x00,
    0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11, 0x80, 0x00, 0x00, 0x00,
    0x01, 0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A,
    0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
];

/// [`H264_KEYFRAME`]. With a broken SPS, the detector rejects it.
fn h264_keyframe(valid_sps: bool) -> Vec<u8> {
    let mut sample = H264_KEYFRAME.to_vec();
    if !valid_sps {
        // `profile_idc`, the SPS's first byte after its NAL header.
        sample[5] = 0x00;
    }
    sample
}

fn sample_batch(keyframe: bool, len: usize) -> re_types_core::SerializedComponentBatch {
    vec![VideoSample::from(vp8_sample(keyframe, len))]
        .try_serialized(VideoStream::descriptor_sample())
        .unwrap()
}

/// One sample per `(time, is keyframe)` frame, on `frame`.
fn video_chunk_len(id: u128, entity: &str, frames: &[(i64, bool)], len: usize) -> Arc<Chunk> {
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
    for (i, &(time, keyframe)) in frames.iter().enumerate() {
        builder = builder.with_serialized_batches(
            row(id, i as u128 + 1),
            [(frame(), time)],
            [sample_batch(keyframe, len)],
        );
    }
    Arc::new(builder.build().unwrap())
}

fn video_chunk(id: u128, entity: &str, frames: &[(i64, bool)]) -> Arc<Chunk> {
    video_chunk_len(id, entity, frames, SAMPLE_LEN)
}

fn codec_chunk(id: u128, entity: &str, codec: VideoCodec) -> Arc<Chunk> {
    Arc::new(
        Chunk::builder_with_id(ChunkId::from_u128(id), entity)
            .with_serialized_batches(
                RowId::from_u128(id << 32),
                TimePoint::default(),
                [vec![codec]
                    .try_serialized(VideoStream::descriptor_codec())
                    .unwrap()],
            )
            .build()
            .unwrap(),
    )
}

/// One VP8 keyframe per codec on `/video`, that codec in the same row, at times `0..` on `frame`.
fn video_chunk_with_codecs(id: u128, codecs: &[VideoCodec]) -> Arc<Chunk> {
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), "video");
    for (i, &codec) in codecs.iter().enumerate() {
        builder = builder.with_serialized_batches(
            row(id, i as u128 + 1),
            [(frame(), i64::try_from(i).unwrap())],
            [
                sample_batch(true, SAMPLE_LEN),
                vec![codec]
                    .try_serialized(VideoStream::descriptor_codec())
                    .unwrap(),
            ],
        );
    }
    Arc::new(builder.build().unwrap())
}

/// A `VideoStream:codec` cell alone on `/video`, at `time` on `frame`.
fn temporal_codec_chunk(id: u128, time: i64, codec: VideoCodec) -> Arc<Chunk> {
    Arc::new(
        Chunk::builder_with_id(ChunkId::from_u128(id), "video")
            .with_serialized_batches(
                row(id, 1),
                [(frame(), time)],
                [vec![codec]
                    .try_serialized(VideoStream::descriptor_codec())
                    .unwrap()],
            )
            .build()
            .unwrap(),
    )
}

/// A VP8 stream on `/video`: the static codec (chunk 1), then `n` one-row chunks, chunk `10 + t`
/// at time `t`, a keyframe every `period` frames.
fn stream(n: i64, period: i64) -> Vec<Arc<Chunk>> {
    let mut chunks = vec![codec_chunk(1, "video", VideoCodec::VP8)];
    chunks.extend((0..n).map(|t| video_chunk(10 + t as u128, "video", &[(t, t % period == 0)])));
    chunks
}

/// A byte target two GoPs of `inputs` fit under and three do not.
fn two_gop_target(inputs: &[Arc<Chunk>]) -> u64 {
    let gops = sample_chunks(&run(
        inputs,
        &gop_settings(1 << 40, vec![passthrough_rule()]),
    ));
    gops.iter().map(measured).max().unwrap() * 5 / 2
}

fn passthrough_rule() -> GopBatchingRule {
    GopBatchingRule {
        samples: MergeSplitOverride::Passthrough,
        ..GopBatchingRule::default()
    }
}

fn gop_settings(max_bytes: u64, rules: Vec<GopBatchingRule>) -> OptimizationSettings {
    OptimizationSettings {
        gop_batching: rules,
        ..settings(max_bytes, 0)
    }
}

fn run(inputs: &[Arc<Chunk>], settings: &OptimizationSettings) -> Vec<Arc<Chunk>> {
    let outputs = collect(optimize(provider_of(inputs.to_vec()), settings).unwrap());
    for chunk in &outputs {
        chunk.sanity_check().unwrap();
    }
    outputs
}

/// Every item of the stream, which ends at its first error.
fn run_to_error(inputs: Vec<Arc<Chunk>>) -> Vec<Result<Arc<Chunk>, Error>> {
    let settings = gop_settings(1 << 40, vec![GopBatchingRule::default()]);
    block_on(optimize(provider_of(inputs), &settings).unwrap().collect())
}

// --- Checks ---

fn sample_chunks(chunks: &[Arc<Chunk>]) -> Vec<Arc<Chunk>> {
    chunks
        .iter()
        .filter(|chunk| chunk.components().contains_component(sample_column()))
        .cloned()
        .collect()
}

/// The chunks holding `is_keyframe` alone.
fn is_keyframe_chunks(chunks: &[Arc<Chunk>]) -> Vec<Arc<Chunk>> {
    chunks
        .iter()
        .filter(|chunk| columns_of(chunk) == BTreeSet::from([keyframe_column()]))
        .cloned()
        .collect()
}

fn num_rows(chunks: &[Arc<Chunk>]) -> Vec<usize> {
    chunks.iter().map(|chunk| chunk.num_rows()).collect()
}

fn row_id_set(chunk: &Chunk) -> BTreeSet<RowId> {
    chunk.row_ids().collect()
}

/// `(time, row id)` of every row of `chunk`, sorted: the order GoP batching cuts in.
fn time_ordered(chunk: &Chunk) -> Vec<(i64, RowId)> {
    let times = chunk.timelines()[&frame_name()].times_raw();
    let mut rows: Vec<(i64, RowId)> =
        std::iter::zip(times.iter().copied(), chunk.row_ids()).collect();
    rows.sort();
    rows
}

/// Row ids of the samples the detector reads as keyframes.
fn detected_keyframes(chunks: &[Arc<Chunk>]) -> BTreeSet<RowId> {
    let mut keyframes = BTreeSet::new();
    for chunk in chunks {
        let Some(list_array) = chunk.components().get_array(sample_column()) else {
            continue;
        };
        let valid_rows = (0..chunk.num_rows()).filter(|&row| list_array.is_valid(row));
        let row_ids = chunk.row_ids_slice();
        for (row, batch) in std::iter::zip(valid_rows, chunk.iter_slices::<&[u8]>(sample_column()))
        {
            let is_keyframe = batch.first().is_some_and(|sample| {
                re_video::detect_gop_start(sample, re_video::VideoCodec::VP8)
                    .unwrap()
                    .is_start_of_gop()
            });
            if is_keyframe {
                keyframes.insert(row_ids[row]);
            }
        }
    }
    keyframes
}

/// Row ids with a `true` `is_keyframe` cell, wherever the column is.
fn marked_keyframes(chunks: &[Arc<Chunk>]) -> BTreeSet<RowId> {
    let mut keyframes = BTreeSet::new();
    for chunk in chunks {
        let Some(list_array) = chunk.components().get_array(keyframe_column()) else {
            continue;
        };
        let valid_rows = (0..chunk.num_rows()).filter(|&row| list_array.is_valid(row));
        let row_ids = chunk.row_ids_slice();
        for (row, batch) in std::iter::zip(
            valid_rows,
            chunk.iter_component::<IsKeyframe>(keyframe_column()),
        ) {
            if batch.as_slice().first().is_some_and(|kf| bool::from(kf.0)) {
                keyframes.insert(row_ids[row]);
            }
        }
    }
    keyframes
}

/// What every GoP-batched output upholds: every cell but `is_keyframe` and the codec survives, the
/// codec is a single static cell, the `true` `is_keyframe` cells are exactly the detected
/// keyframes, sample chunks hold no other column than an inline `is_keyframe`, and every sample
/// chunk starts at a keyframe, or before the first keyframe when the stream does not open on one.
fn assert_gop_batched(inputs: &[Arc<Chunk>], outputs: &[Arc<Chunk>]) {
    let without_regenerated = |chunks: &[Arc<Chunk>]| -> BTreeSet<_> {
        cell_set(chunks)
            .into_iter()
            .filter(|(_, _, column)| *column != keyframe_column() && *column != codec_column())
            .collect()
    };
    assert_eq!(without_regenerated(inputs), without_regenerated(outputs));

    let codec_chunks: Vec<&Arc<Chunk>> = outputs
        .iter()
        .filter(|chunk| chunk.components().contains_component(codec_column()))
        .collect();
    assert!(
        matches!(codec_chunks.as_slice(), [chunk] if chunk.is_static() && chunk.num_rows() == 1),
        "the codec must come out as a single static cell: {codec_chunks:?}"
    );

    let keyframes = detected_keyframes(inputs);
    assert_eq!(marked_keyframes(outputs), keyframes);

    let samples = sample_chunks(outputs);
    for chunk in &samples {
        let mut columns = columns_of(chunk);
        columns.remove(&keyframe_column());
        assert_eq!(columns, BTreeSet::from([sample_column()]));
    }
    let first_keyframe = samples
        .iter()
        .flat_map(|chunk| time_ordered(chunk))
        .filter(|(_, row_id)| keyframes.contains(row_id))
        .min();
    for chunk in &samples {
        let head = time_ordered(chunk)[0];
        assert!(
            keyframes.contains(&head.1) || first_keyframe.is_none_or(|first| head < first),
            "sample chunk does not start at a keyframe: {head:?}"
        );
    }
}

// --- Tests ---

/// Whole GoPs pack toward the target: two per output and the last alone. `Passthrough` emits one
/// chunk per GoP.
#[test]
fn gop_packing() {
    let inputs = stream(20, 4);

    let outputs = run(&inputs, &gop_settings(1 << 40, vec![passthrough_rule()]));
    assert_gop_batched(&inputs, &outputs);
    let gops = sample_chunks(&outputs);
    assert_eq!(num_rows(&gops), vec![4; 5]);

    let outputs = run(
        &inputs,
        &gop_settings(two_gop_target(&inputs), vec![GopBatchingRule::default()]),
    );
    assert_gop_batched(&inputs, &outputs);
    assert_eq!(num_rows(&sample_chunks(&outputs)), vec![8, 8, 4]);
}

/// A GoP above the byte target, or above the row guard, comes out whole.
#[test]
fn gop_oversized_emits_whole() {
    let inputs = stream(8, 8);
    let rule = || vec![GopBatchingRule::default()];
    for settings in [
        gop_settings(1024, rule()),
        OptimizationSettings {
            gop_batching: rule(),
            ..settings_max_row_unsorted(1 << 40, 2, 0)
        },
    ] {
        let outputs = run(&inputs, &settings);
        assert_gop_batched(&inputs, &outputs);
        assert_eq!(num_rows(&sample_chunks(&outputs)), vec![8]);
    }
}

/// Two inputs interleaved in time, one of them time-unsorted within itself: GoPs gather rows from
/// both, in time order, with either keyframe placement.
#[test]
fn gop_reorders_overlapping_inputs() {
    let inputs = vec![
        video_chunk(1, "video", &[(0, true), (20, true), (10, false)]),
        video_chunk(2, "video", &[(5, false), (15, false), (25, false)]),
        codec_chunk(3, "video", VideoCodec::VP8),
    ];
    assert!(!inputs[0].is_timeline_sorted(&frame_name()));

    for keyframes in [KeyframePlacement::Inline, KeyframePlacement::OwnChunk(None)] {
        let rule = GopBatchingRule {
            keyframes,
            ..passthrough_rule()
        };
        let outputs = run(&inputs, &gop_settings(1 << 40, vec![rule]));
        assert_gop_batched(&inputs, &outputs);
        let gops: Vec<Vec<RowId>> = sample_chunks(&outputs)
            .iter()
            .map(|chunk| time_ordered(chunk).into_iter().map(|(_, id)| id).collect())
            .collect();
        assert_eq!(
            gops,
            vec![
                vec![row(1, 1), row(2, 1), row(1, 3), row(2, 2)],
                vec![row(1, 2), row(2, 3)],
            ]
        );
    }
}

/// A stream that does not open on a keyframe: the rows before the first keyframe come out first,
/// unmarked, with either keyframe placement.
#[test]
fn gop_orphan_head() {
    let mut inputs = vec![codec_chunk(1, "video", VideoCodec::VP8)];
    inputs.extend((1..=8).map(|t| video_chunk(9 + t as u128, "video", &[(t, t % 4 == 0)])));

    for keyframes in [KeyframePlacement::Inline, KeyframePlacement::OwnChunk(None)] {
        let rule = GopBatchingRule {
            keyframes,
            ..passthrough_rule()
        };
        let outputs = run(&inputs, &gop_settings(1 << 40, vec![rule]));
        assert_gop_batched(&inputs, &outputs);
        let samples = sample_chunks(&outputs);
        assert_eq!(num_rows(&samples), vec![3, 4, 1]);
        assert_eq!(
            row_id_set(&samples[0]),
            BTreeSet::from([row(10, 1), row(11, 1), row(12, 1)])
        );
    }
}

/// A stream with no detected keyframe is not buffered whole: its rows come out as each IO batch
/// makes them final.
#[test]
fn gop_keyframeless_stream() {
    // Each chunk measures above the IO batch floor, so each is an IO batch of its own.
    let big = 3 * 1024 * 1024;
    let mut inputs = vec![codec_chunk(1, "video", VideoCodec::VP8)];
    inputs.extend((0..3_i64).map(|i| {
        let frames: Vec<(i64, bool)> = (0..3).map(|t| (3 * i + t, false)).collect();
        video_chunk_len(10 + i as u128, "video", &frames, big)
    }));

    let outputs = run(&inputs, &gop_settings(1 << 40, vec![passthrough_rule()]));

    assert_gop_batched(&inputs, &outputs);
    assert_eq!(num_rows(&sample_chunks(&outputs)), vec![3, 3, 3]);
}

/// A row with a null sample stays out of the GoPs, with the entity's other columns, an empty sample
/// batch is not a keyframe, and a row with no cell at all is not emitted.
#[test]
fn gop_null_sample_rows() {
    let point = MyPoint::from_iter(0..1)
        .try_serialized(MyPoints::descriptor_points())
        .unwrap();
    let empty_sample = Vec::<VideoSample>::new()
        .try_serialized(VideoStream::descriptor_sample())
        .unwrap();
    let rows = [
        (0, Some(sample_batch(true, SAMPLE_LEN)), None),
        (1, Some(sample_batch(false, SAMPLE_LEN)), None),
        (2, None, Some(point)),
        (3, Some(empty_sample), None),
        (4, None, None),
        (5, Some(sample_batch(true, SAMPLE_LEN)), None),
        (6, Some(sample_batch(false, SAMPLE_LEN)), None),
    ];
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(1), "video");
    for (i, (time, sample, point)) in rows.into_iter().enumerate() {
        builder = builder.with_sparse_serialized_batches(
            row(1, i as u128 + 1),
            [(frame(), time)],
            [
                (VideoStream::descriptor_sample(), sample),
                (MyPoints::descriptor_points(), point),
            ],
        );
    }
    let inputs = vec![
        Arc::new(builder.build().unwrap()),
        codec_chunk(2, "video", VideoCodec::VP8),
    ];

    let outputs = run(&inputs, &gop_settings(1 << 40, vec![passthrough_rule()]));
    assert_gop_batched(&inputs, &outputs);
    assert_eq!(
        sample_chunks(&outputs)
            .iter()
            .map(|chunk| row_id_set(chunk))
            .collect::<Vec<_>>(),
        vec![
            BTreeSet::from([row(1, 1), row(1, 2), row(1, 4)]),
            BTreeSet::from([row(1, 6), row(1, 7)]),
        ]
    );
    assert!(!row_set(&outputs).contains(&("video".to_owned(), row(1, 5))));
}

/// An all-null `is_keyframe` column is invisible to the chunk index, so the planner cannot route
/// it; the run drops it anyway.
#[test]
fn gop_all_null_keyframe_column() {
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(1), "video");
    for (i, keyframe) in [true, false, true, false].into_iter().enumerate() {
        builder = builder.with_sparse_serialized_batches(
            row(1, i as u128 + 1),
            [(frame(), i64::try_from(i).unwrap())],
            [
                (
                    VideoStream::descriptor_sample(),
                    Some(sample_batch(keyframe, SAMPLE_LEN)),
                ),
                (VideoStream::descriptor_is_keyframe(), None),
            ],
        );
    }
    let datatypes: IntMap<_, _> = std::iter::once((
        VideoStream::descriptor_is_keyframe(),
        IsKeyframe::arrow_data_type(),
    ))
    .collect();
    let chunk = Arc::new(builder.build_with_datatypes(&datatypes).unwrap());
    assert!(chunk.components().contains_component(keyframe_column()));
    let inputs = vec![chunk, codec_chunk(2, "video", VideoCodec::VP8)];

    let outputs = run(
        &inputs,
        &gop_settings(1 << 40, vec![GopBatchingRule::default()]),
    );
    assert_gop_batched(&inputs, &outputs);
}

/// Logged `is_keyframe` values, next to the samples or in chunks of their own, are dropped: the
/// output holds the detector's `is_keyframe` cells alone.
#[test]
fn gop_drops_logged_keyframes() {
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(2), "video");
    for i in 0..4_u128 {
        builder = builder.with_serialized_batches(
            row(2, i + 1),
            [(frame(), i as i64)],
            [
                sample_batch(i == 0, SAMPLE_LEN),
                vec![IsKeyframe::from(true)]
                    .try_serialized(VideoStream::descriptor_is_keyframe())
                    .unwrap(),
            ],
        );
    }
    let next_to_samples = Arc::new(builder.build().unwrap());
    let dedicated = Arc::new(
        Chunk::builder_with_id(ChunkId::from_u128(4), "video")
            .with_serialized_batches(
                row(4, 1),
                [(frame(), 1)],
                [vec![IsKeyframe::from(true)]
                    .try_serialized(VideoStream::descriptor_is_keyframe())
                    .unwrap()],
            )
            .build()
            .unwrap(),
    );
    let inputs = vec![
        codec_chunk(1, "video", VideoCodec::VP8),
        next_to_samples,
        dedicated,
    ];

    let outputs = run(
        &inputs,
        &gop_settings(1 << 40, vec![GopBatchingRule::default()]),
    );
    assert_gop_batched(&inputs, &outputs);
}

/// `Inline` adds `is_keyframe` to the sample chunks. `OwnChunk` packs the `is_keyframe` cells into
/// chunks of their own, toward the resolved target.
#[test]
fn gop_keyframe_placement() {
    let inputs = stream(20, 4);

    let inline = GopBatchingRule {
        keyframes: KeyframePlacement::Inline,
        ..GopBatchingRule::default()
    };
    let outputs = run(&inputs, &gop_settings(1 << 40, vec![inline]));
    assert_gop_batched(&inputs, &outputs);
    assert!(is_keyframe_chunks(&outputs).is_empty());
    let non_null: usize = sample_chunks(&outputs)
        .iter()
        .map(|chunk| {
            let column = chunk.components().get_array(keyframe_column()).unwrap();
            column.len() - column.null_count()
        })
        .sum();
    assert_eq!(non_null, 5, "`true` on the keyframes, null elsewhere");

    let is_keyframe_chunk_count = |target| {
        let rule = GopBatchingRule {
            keyframes: KeyframePlacement::OwnChunk(Some(target)),
            ..GopBatchingRule::default()
        };
        let outputs = run(&inputs, &gop_settings(1 << 40, vec![rule]));
        assert_gop_batched(&inputs, &outputs);
        assert!(
            sample_chunks(&outputs)
                .iter()
                .all(|chunk| !chunk.components().contains_component(keyframe_column()))
        );
        is_keyframe_chunks(&outputs).len()
    };
    assert_eq!(is_keyframe_chunk_count(merge_split_settings(1)), 5);
    assert_eq!(is_keyframe_chunk_count(merge_split_settings(1 << 40)), 1);
}

/// A stream GoP batching cannot handle fails the optimization. Codecs that disagree, with no
/// static codec to shadow them, fail before the stream's first output where they are read up
/// front, and later otherwise; so does a sample datatype the reader does not handle.
#[test]
fn gop_errors() {
    let one_sample = |sample: ArrayRef| {
        Arc::new(
            Chunk::builder_with_id(ChunkId::from_u128(2), "video")
                .with_row(
                    row(2, 1),
                    [(frame(), 0_i64)],
                    [(VideoStream::descriptor_sample(), sample)],
                )
                .build()
                .unwrap(),
        )
    };

    // Read up front: within the first sample chunk, or across the codec-only chunks.
    for inputs in [
        vec![video_chunk_with_codecs(
            2,
            &[VideoCodec::VP8, VideoCodec::VP9],
        )],
        vec![
            temporal_codec_chunk(1, 0, VideoCodec::VP8),
            temporal_codec_chunk(3, 1, VideoCodec::VP9),
            video_chunk(2, "video", &[(0, true)]),
        ],
    ] {
        let results = run_to_error(inputs);
        assert!(
            matches!(
                results.as_slice(),
                [Err(Error::VideoStream {
                    source: VideoStreamErrorSource::SeveralCodecs {
                        first: VideoCodec::VP8,
                        second: VideoCodec::VP9,
                    },
                    ..
                })]
            ),
            "{results:?}"
        );
    }

    let results = run_to_error(vec![
        temporal_codec_chunk(1, 0, VideoCodec::VP8),
        video_chunk_with_codecs(2, &[VideoCodec::VP9]),
    ]);
    assert!(
        matches!(
            results.last(),
            Some(Err(Error::VideoStream {
                source: VideoStreamErrorSource::CodecMismatch {
                    found: VideoCodec::VP9,
                    expected: VideoCodec::VP8,
                },
                ..
            }))
        ),
        "{results:?}"
    );

    let results = run_to_error(vec![
        codec_chunk(1, "video", VideoCodec::VP8),
        one_sample(Arc::new(UInt32Array::from(vec![1, 2, 3]))),
    ]);
    assert!(
        matches!(
            results.last(),
            Some(Err(Error::VideoStream {
                source: VideoStreamErrorSource::UnsupportedSampleDatatype(DataType::UInt32),
                ..
            }))
        ),
        "{results:?}"
    );
}

/// The stream comes out with a single static codec. A static codec shadows the temporal codec
/// cells, even disagreeing ones, and is kept as is; without one, the temporal codec becomes a new
/// static cell.
#[test]
fn gop_single_static_codec() {
    let settings = gop_settings(1 << 40, vec![GopBatchingRule::default()]);
    let static_codecs = |outputs: &[Arc<Chunk>]| -> Vec<(RowId, VideoCodec)> {
        outputs
            .iter()
            .filter(|chunk| chunk.is_static())
            .flat_map(|chunk| {
                std::iter::zip(
                    chunk.row_ids(),
                    chunk
                        .iter_component::<VideoCodec>(codec_column())
                        .map(|batch| batch.as_slice()[0]),
                )
            })
            .collect()
    };

    let shadowed = vec![
        codec_chunk(1, "video", VideoCodec::VP8),
        video_chunk_with_codecs(2, &[VideoCodec::VP9, VideoCodec::VP9]),
    ];
    let outputs = run(&shadowed, &settings);
    assert_gop_batched(&shadowed, &outputs);
    assert_eq!(
        static_codecs(&outputs),
        vec![(RowId::from_u128(1 << 32), VideoCodec::VP8)]
    );

    let temporal = vec![video_chunk_with_codecs(
        2,
        &[VideoCodec::VP8, VideoCodec::VP8],
    )];
    let outputs = run(&temporal, &settings);
    assert_gop_batched(&temporal, &outputs);
    let [(row_id, codec)] = static_codecs(&outputs)[..] else {
        panic!("expected one static codec cell");
    };
    assert_eq!(codec, VideoCodec::VP8);
    assert!(!row_id_set(&temporal[0]).contains(&row_id), "a new row id");
}

/// A sample the detector rejects, the first one included, does not fail the run: it survives,
/// unmarked, inside the GoP of the keyframe before it, or ahead of the first keyframe.
#[test]
fn gop_rejected_sample() {
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(2), "video");
    for (i, valid_sps) in [false, true, false, true].into_iter().enumerate() {
        builder = builder.with_serialized_batches(
            row(2, i as u128 + 1),
            [(frame(), i64::try_from(i).unwrap())],
            [vec![VideoSample::from(h264_keyframe(valid_sps))]
                .try_serialized(VideoStream::descriptor_sample())
                .unwrap()],
        );
    }
    let inputs = vec![
        codec_chunk(1, "video", VideoCodec::H264),
        Arc::new(builder.build().unwrap()),
    ];

    let outputs = run(&inputs, &gop_settings(1 << 40, vec![passthrough_rule()]));

    let gops: Vec<BTreeSet<RowId>> = sample_chunks(&outputs)
        .iter()
        .map(|chunk| row_id_set(chunk))
        .collect();
    assert_eq!(
        gops,
        vec![
            BTreeSet::from([row(2, 1)]),
            BTreeSet::from([row(2, 2), row(2, 3)]),
            BTreeSet::from([row(2, 4)])
        ]
    );
    assert_eq!(
        marked_keyframes(&outputs),
        BTreeSet::from([row(2, 2), row(2, 4)])
    );
}

/// Rows at the time where the next unloaded input starts wait for it: that input can hold a row
/// with a smaller row id at the same time.
#[test]
fn gop_frontier_across_batches() {
    // Each chunk measures above the IO batch floor, so each is an IO batch of its own.
    let big = 3 * 1024 * 1024;
    let a = video_chunk_len(2, "video", &[(0, true), (10, true), (20, true)], big);
    let b = video_chunk_len(1, "video", &[(10, false), (30, true), (50, false)], big);
    let c = video_chunk_len(3, "video", &[(50, false), (60, true), (70, false)], big);
    assert!(
        [&a, &b, &c]
            .iter()
            .all(|chunk| measured(chunk) > 8 * 1024 * 1024)
    );
    let inputs = vec![a, b, c, codec_chunk(4, "video", VideoCodec::VP8)];

    let outputs = run(&inputs, &gop_settings(1 << 40, vec![passthrough_rule()]));
    assert_gop_batched(&inputs, &outputs);
    assert_eq!(
        sample_chunks(&outputs)
            .iter()
            .map(|chunk| row_id_set(chunk))
            .collect::<Vec<_>>(),
        vec![
            BTreeSet::from([row(2, 1), row(1, 1)]),
            BTreeSet::from([row(2, 2)]),
            BTreeSet::from([row(2, 3)]),
            BTreeSet::from([row(1, 2), row(1, 3), row(3, 1)]),
            BTreeSet::from([row(3, 2), row(3, 3)]),
        ]
    );
}

/// Re-optimizing the output gives the same layout.
#[test]
fn gop_stable_layout() {
    let inputs = stream(20, 4);
    let settings = gop_settings(two_gop_target(&inputs), vec![GopBatchingRule::default()]);
    let heads = |outputs: &[Arc<Chunk>]| {
        sample_chunks(outputs)
            .iter()
            .map(|chunk| time_ordered(chunk)[0])
            .collect::<Vec<_>>()
    };

    let pass_1 = run(&inputs, &settings);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pass1.rrd");
    let store_id = test_store_id();
    write_rrd(&path, &store_id, &pass_1);
    let pass_2 = collect(optimize(file_provider(&path, &store_id), &settings).unwrap());

    assert_eq!(pass_1.len(), pass_2.len());
    assert_eq!(heads(&pass_1), heads(&pass_2));
    assert_eq!(cell_set(&pass_1), cell_set(&pass_2));
}

/// With `merge_split: None`, video is still cut at keyframes, and every other chunk passes
/// through untouched.
#[test]
fn gop_passthrough_global() {
    let other = temporal_point_chunk(2, "other", &[0, 1], 4);
    let mut inputs = stream(12, 4);
    inputs.push(other.clone());
    let settings = OptimizationSettings {
        merge_split: None,
        ..gop_settings(1, vec![GopBatchingRule::default()])
    };

    let outputs = run(&inputs, &settings);
    assert_gop_batched(&inputs, &outputs);
    assert_eq!(num_rows(&sample_chunks(&outputs)), vec![4; 3]);
    for passthrough in [&inputs[0], &other] {
        assert!(outputs.iter().any(|chunk| Arc::ptr_eq(chunk, passthrough)));
    }
}
