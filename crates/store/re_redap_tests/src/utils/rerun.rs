use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::{ArrayRef, FixedSizeListArray, Float32Array};
use arrow::datatypes::Field;
use itertools::Itertools as _;
use re_log_types::{TimePoint, TimeType, Timeline, build_index_value};
use re_sdk::RecordingStreamBuilder;
use re_tuid::Tuid;
use re_types_core::AsComponents;

use crate::TempPath;

// ---

/// Indicates the prefix used for all `Tuid`s in a given recording, i.e.
/// ```ignore
/// Tuid::from_nanos_and_inc(TuidPrefix, 0)
/// ```
pub type TuidPrefix = u64;

pub fn next_chunk_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::ChunkId {
    let mut chunk_id = re_chunk::ChunkId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
    move || {
        chunk_id = chunk_id.next();
        chunk_id
    }
}

pub fn next_row_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::RowId {
    let mut row_id = re_chunk::RowId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
    move || {
        row_id = row_id.next();
        row_id
    }
}

// ---

/// Creates a simple, clean recording with sequential data and no overlaps or duplicates.
///
/// This recording is made such that it cannot be compacted, so that the effect of compaction is
/// ruled out in snapshots.
pub fn create_simple_recording(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    entity_paths: &[&str],
    time_type: TimeType,
) -> anyhow::Result<TempPath> {
    let tmp_dir = tempfile::tempdir()?;
    let path = create_simple_recording_in(
        tuid_prefix,
        segment_id,
        entity_paths,
        time_type,
        tmp_dir.path(),
    )?;
    Ok(TempPath::new(tmp_dir, path))
}

/// Creates a simple, clean recording with sequential data and no overlaps or duplicates and save
/// it to `in_dir`.
///
/// This recording is made such that it cannot be compacted, so that the effect of compaction is
/// ruled out in snapshots. The `in_dir` is assumed to exist and not deleted automatically.
pub fn create_simple_recording_in(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    entity_paths: &[&str],
    time_type: TimeType,
    in_dir: &std::path::Path,
) -> anyhow::Result<PathBuf> {
    use re_chunk::{Chunk, TimePoint};
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::{EntityPath, TimeInt};

    if !std::fs::metadata(in_dir)?.is_dir() {
        return Err(anyhow::anyhow!("Expected `in_dir` to be a directory"));
    }

    let tmp_path = in_dir.join(format!("{segment_id}.rrd"));

    let rec = RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    for entity_path in entity_paths {
        let entity_path = EntityPath::from(*entity_path);

        // Sequential frames
        let frame1 = TimeInt::new_temporal(10);
        let frame2 = TimeInt::new_temporal(20);
        let frame3 = TimeInt::new_temporal(30);
        let frame4 = TimeInt::new_temporal(40);

        // Data for each frame
        let points1 = MyPoint::from_iter(0..1);
        let points2 = MyPoint::from_iter(1..2);
        let points3 = MyPoint::from_iter(2..3);
        let points4 = MyPoint::from_iter(3..4);

        let colors1 = MyColor::from_iter(0..1);
        let colors2 = MyColor::from_iter(1..2);
        let colors3 = MyColor::from_iter(2..3);
        let colors4 = MyColor::from_iter(3..4);

        let labels = vec![MyLabel("simple".to_owned())];

        // Single chunk with sequential, complete data
        let chunk = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_index_value(frame1, time_type)],
                [
                    (MyPoints::descriptor_points(), Some(&points1 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors1 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_index_value(frame2, time_type)],
                [
                    (MyPoints::descriptor_points(), Some(&points2 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors2 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_index_value(frame3, time_type)],
                [
                    (MyPoints::descriptor_points(), Some(&points3 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_index_value(frame4, time_type)],
                [
                    (MyPoints::descriptor_points(), Some(&points4 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors4 as _)),
                ],
            )
            .build()?;

        rec.send_chunk(chunk);

        let static_chunk = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyPoints::descriptor_labels(), Some(&labels as _))],
            )
            .build()?;

        rec.send_chunk(static_chunk);
    }

    rec.flush_blocking()?;

    Ok(tmp_path)
}

/// Creates a simple blueprint.
pub fn create_simple_blueprint(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
) -> anyhow::Result<TempPath> {
    use re_chunk::Chunk;
    use re_log_types::{EntityPath, TimeInt, build_frame_nr};
    use re_sdk_types::blueprint::archetypes::TimePanelBlueprint;

    let tmp_path = {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(format!("{segment_id}.rbl"));
        TempPath::new(dir, path)
    };

    let rec = RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .blueprint()
        .recording_id(segment_id)
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    let chunk = Chunk::builder_with_id(next_chunk_id(), EntityPath::from("/time_panel"))
        .with_archetype(
            next_row_id(),
            [build_frame_nr(TimeInt::new_temporal(0))],
            &TimePanelBlueprint::default().with_fps(60.0),
        )
        .build()?;

    rec.send_chunk(chunk);

    rec.flush_blocking()?;

    Ok(tmp_path)
}

/// Creates a very nasty recording with all kinds of partial updates, chunk overlaps, repeated
/// timestamps, duplicated chunks, partial multi-timelines, flat and recursive clears, etc.
///
/// This makes it a great recording to test things with for most situations.
pub fn create_nasty_recording(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    entity_paths: &[&str],
) -> anyhow::Result<TempPath> {
    use re_chunk::{Chunk, TimePoint};
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::{EntityPath, TimeInt, build_frame_nr, build_log_time};

    let tmp_path = {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(format!("{segment_id}.rrd"));
        TempPath::new(dir, path)
    };

    let rec = RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        // NOTE: Don't send builtin properties (e.g. recording start time): these are non
        // deterministic (neither their values nor their Chunk/Row IDs) and are not what we're
        // trying to test anyhow. We have dedicated, in-depth deterministic test suites for properties.
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    /// So we can test duration-based indexes too.
    fn build_sim_time(t: impl TryInto<TimeInt>) -> (Timeline, TimeInt) {
        (
            Timeline::new("sim_time", TimeType::DurationNs),
            TimeInt::saturated_temporal(t),
        )
    }

    for entity_path in entity_paths {
        let entity_path = EntityPath::from(*entity_path);

        let frame1 = TimeInt::new_temporal(10);
        let frame2 = TimeInt::new_temporal(20);
        let frame3 = TimeInt::new_temporal(30);
        let frame4 = TimeInt::new_temporal(40);
        let frame5 = TimeInt::new_temporal(50);
        let frame6 = TimeInt::new_temporal(60);
        let frame7 = TimeInt::new_temporal(70);

        let points1 = MyPoint::from_iter(0..1);
        let points2 = MyPoint::from_iter(1..2);
        let points3 = MyPoint::from_iter(2..3);
        let points4 = MyPoint::from_iter(3..4);
        let points5 = MyPoint::from_iter(4..5);
        let points6 = MyPoint::from_iter(5..6);
        let points7_1 = MyPoint::from_iter(6..7);
        let points7_2 = MyPoint::from_iter(7..8);
        let points7_3 = MyPoint::from_iter(8..9);

        let colors3 = MyColor::from_iter(2..3);
        let colors4 = MyColor::from_iter(3..4);
        let colors5 = MyColor::from_iter(4..5);
        let colors7 = MyColor::from_iter(6..7);

        let labels1 = vec![MyLabel("a".to_owned())];
        let labels2 = vec![MyLabel("b".to_owned())];
        let labels3 = vec![MyLabel("c".to_owned())];

        let chunk1_1 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame1),
                    build_log_time(frame1.into()),
                    build_sim_time(frame1),
                ],
                [
                    (MyPoints::descriptor_points(), Some(&points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(&labels1 as _)), // shadowed by static
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame3),
                    build_log_time(frame3.into()),
                    build_sim_time(frame3),
                ],
                [
                    (MyPoints::descriptor_points(), Some(&points3 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame5),
                    build_log_time(frame5.into()),
                    build_sim_time(frame5),
                ],
                [
                    (MyPoints::descriptor_points(), Some(&points5 as _)),
                    (MyPoints::descriptor_colors(), None),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoints::descriptor_points(), Some(&points7_1 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoints::descriptor_points(), Some(&points7_2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoints::descriptor_points(), Some(&points7_3 as _))],
            )
            .build()?;
        let chunk1_2 = chunk1_1.clone_as(next_chunk_id(), next_row_id());
        let chunk1_3 = chunk1_1.clone_as(next_chunk_id(), next_row_id());

        rec.send_chunk(chunk1_1);
        rec.send_chunk(chunk1_2); // x2!
        rec.send_chunk(chunk1_3); // x3!

        let chunk2 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame2)],
                [(MyPoints::descriptor_points(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame3)],
                [
                    (MyPoints::descriptor_points(), Some(&points3 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_points(), Some(&points4 as _))],
            )
            .build()?;

        rec.send_chunk(chunk2);

        let chunk3 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame2)],
                [(MyPoints::descriptor_points(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_points(), Some(&points4 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame6)],
                [(MyPoints::descriptor_points(), Some(&points6 as _))],
            )
            .build()?;

        rec.send_chunk(chunk3);

        let chunk4 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_colors(), Some(&colors4 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame5)],
                [(MyPoints::descriptor_colors(), Some(&colors5 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame7)],
                [(MyPoints::descriptor_colors(), Some(&colors7 as _))],
            )
            .build()?;

        rec.send_chunk(chunk4);

        let chunk5 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyPoints::descriptor_labels(), Some(&labels2 as _))],
            )
            .build()?;

        rec.send_chunk(chunk5);

        let chunk6 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyPoints::descriptor_labels(), Some(&labels3 as _))],
            )
            .build()?;

        rec.send_chunk(chunk6);
    }

    for entity_path in entity_paths {
        let entity_path = EntityPath::from(*entity_path);

        let frame95 = TimeInt::new_temporal(950);
        let frame99 = TimeInt::new_temporal(990);

        let colors99 = MyColor::from_iter(99..100);

        let labels95 = vec![MyLabel("z".to_owned())];

        let chunk7 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame99)],
                [(MyPoints::descriptor_colors(), Some(&colors99 as _))],
            )
            .build()?;

        rec.send_chunk(chunk7);

        let chunk8 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame95)],
                [(MyPoints::descriptor_labels(), Some(&labels95 as _))],
            )
            .build()?;

        rec.send_chunk(chunk8);
    }

    rec.flush_blocking()?;

    Ok(tmp_path)
}

/// Create an rrd recording with embeddings with 256 floats each. Total number of embeddings (rows)
/// and number of embeddings per row can be specified.
///
/// Note that creating a Lance vector index requires at least 256 embeddings, but our index creation
/// won't fail if there are less than that, it will just go through regular search path i.e. won't
/// be optimized by the Lance index.
pub fn create_recording_with_embeddings(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    embeddings: u32,
    embeddings_per_row: u32,
) -> anyhow::Result<TempPath> {
    use re_chunk::Chunk;
    use re_log_types::{TimeInt, build_log_time};
    use re_sdk::{ComponentDescriptor, SerializedComponentBatch};

    let tmp_path = {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(format!("{segment_id}.rrd"));
        TempPath::new(dir, path)
    };

    let rec = re_sdk::RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        // NOTE: Don't send builtin properties (e.g. recording start time): these are non
        // deterministic (neither their values nor their Chunk/Row IDs) and are not what we're
        // trying to test anyhow. We have dedicated, in-depth deterministic test suites for properties.
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    let rows = embeddings.div_ceil(embeddings_per_row);

    for i in 1..=rows {
        let floats_arrays = if embeddings_per_row > 1 {
            let mut row_data = vec![];
            for j in 1..=embeddings_per_row {
                let floats: Vec<f32> = (0..256)
                    .map(|_| 0.1f32 * i as f32 * (j * j) as f32)
                    .collect();
                row_data.push(Arc::new(Float32Array::from(floats)) as ArrayRef);
            }
            let arrays = row_data.iter().map(|a| a.as_ref()).collect_vec();
            let flat = arrow::compute::concat(&arrays).expect("Failed to concatenate arrays");

            let row = FixedSizeListArray::new(
                Arc::new(Field::new(
                    "item",
                    arrow::datatypes::DataType::Float32,
                    true,
                )),
                256,
                Arc::new(flat),
                None, // Or handle nulls appropriately
            );

            Arc::new(row)
        } else {
            let floats: Vec<f32> = (0..256).map(|_| 0.1f32 * (i * i) as f32).collect();
            Arc::new(Float32Array::from(floats)) as ArrayRef
        };

        let frame = TimeInt::new_temporal((i * 10) as i64);

        let chunk = Chunk::builder_with_id(next_chunk_id(), "/my_embeddings")
            .with_serialized_batch(
                next_row_id(),
                [build_log_time(frame.into())],
                SerializedComponentBatch::new(
                    floats_arrays,
                    ComponentDescriptor::partial("embedding"),
                ),
            )
            .build()?;

        rec.send_chunk(chunk);
    }

    // log another set of embeddings with a different entity path
    for i in 1..=rows {
        let floats_arrays = if embeddings_per_row > 1 {
            let mut row_data = vec![];
            for j in 1..=embeddings_per_row {
                let floats: Vec<f32> = (0..384)
                    .map(|_| 0.2f32 * i as f32 * (j * j) as f32)
                    .collect();
                row_data.push(Arc::new(Float32Array::from(floats)) as ArrayRef);
            }
            let arrays = row_data.iter().map(|a| a.as_ref()).collect_vec();
            let flat = arrow::compute::concat(&arrays).expect("Failed to concatenate arrays");

            let row = FixedSizeListArray::new(
                Arc::new(Field::new(
                    "item",
                    arrow::datatypes::DataType::Float32,
                    true,
                )),
                384,
                Arc::new(flat),
                None, // Or handle nulls appropriately
            );

            Arc::new(row)
        } else {
            let floats: Vec<f32> = (0..384).map(|_| 0.2f32 * (i * i) as f32).collect();
            Arc::new(Float32Array::from(floats)) as ArrayRef
        };

        let frame = TimeInt::new_temporal((i * 10) as i64);

        let chunk = Chunk::builder_with_id(next_chunk_id(), "/my_embeddings_long")
            .with_serialized_batch(
                next_row_id(),
                [build_log_time(frame.into())],
                SerializedComponentBatch::new(
                    floats_arrays,
                    // intentionally name similarly to the one above, ensuring we exercise fuzzy descriptor matching logic
                    ComponentDescriptor::partial("embedding_long"),
                ),
            )
            .build()?;

        rec.send_chunk(chunk);
    }

    rec.flush_blocking()?;

    Ok(tmp_path)
}

pub fn create_recording_with_scalars(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    n: usize,
) -> anyhow::Result<TempPath> {
    use re_chunk::Chunk;
    use re_log_types::{TimeInt, build_log_time};
    use re_sdk::{ComponentDescriptor, SerializedComponentBatch};

    let tmp_path = {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(format!("{segment_id}.rrd"));
        TempPath::new(dir, path)
    };

    let rec = re_sdk::RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        // NOTE: Don't send builtin properties (e.g. recording start time): these are non
        // deterministic (neither their values nor their Chunk/Row IDs) and are not what we're
        // trying to test anyhow. We have dedicated, in-depth deterministic test suites for properties.
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    #[expect(clippy::cast_possible_wrap)]
    for i in 1..=n as i64 {
        let floats: Vec<f32> = (0..10).map(|j| 0.1f32 * i as f32 * j as f32).collect();
        let float_array = Arc::new(Float32Array::from(floats)) as ArrayRef;

        let frame = TimeInt::new_temporal(i * 10);

        let chunk = Chunk::builder_with_id(next_chunk_id(), "/my_scalars")
            .with_serialized_batch(
                next_row_id(),
                [build_log_time(frame.into())],
                SerializedComponentBatch::new(float_array, ComponentDescriptor::partial("scalar")),
            )
            .build()?;

        rec.send_chunk(chunk);
    }

    rec.flush_blocking()?;

    Ok(tmp_path)
}

pub fn create_recording_with_text(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
) -> anyhow::Result<TempPath> {
    use re_chunk::Chunk;
    use re_log_types::{TimeInt, build_log_time};

    let tmp_path = {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(format!("{segment_id}.rrd"));
        TempPath::new(dir, path)
    };

    let rec = re_sdk::RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        // NOTE: Don't send builtin properties (e.g. recording start time): these are non
        // deterministic (neither their values nor their Chunk/Row IDs) and are not what we're
        // trying to test anyhow. We have dedicated, in-depth deterministic test suites for properties.
        .send_properties(false)
        .save(tmp_path.clone())?;

    let sentences = [
        "A sagging bookshelf overflows with worn paperbacks.",
        "A weathered signpost points to half-forgotten towns.",
        "Buttercups bloom across the meadow like scattered gold.",
        "A sleepy cat sprawls atop a dusty grand piano.",
        "Aspen leaves quake in a sudden gust of wind.",
        "A weathered wooden cross stands in a silent clearing.",
        "A flickering TV illuminates the lonely motel room.",
        "A hidden path winds through the dense cedar grove.",
        "Songbirds greet the dawn with a cheery chorus.",
        "A wooden cradle rocks gently in the candlelit room.",
        "The horizon blushes with the first light of sunrise.",
        "A donkey's bray pierces the stillness of midday heat.",
        "A red door stands out on a row of faded houses.",
        "Frost patterns lace the window on a frigid morning.",
        "A wide river floods the lowland fields in early spring.",
        "Petals from a wilted bouquet scatter across the steps.",
        "A porcelain doll gazes emptily from a cracked cabinet.",
        "The final bell tolls in the deserted bell tower.",
        "A faint rainbow forms above the rippling waterfall.",
        "Golden hay bales glow under the setting sun's rays.",
    ];

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    for (i, sentence) in sentences.iter().enumerate() {
        #[expect(clippy::cast_possible_wrap)]
        let frame = TimeInt::new_temporal((i * 10) as i64);

        let chunk = Chunk::builder_with_id(next_chunk_id(), "/my_text")
            .with_archetype(
                next_row_id(),
                [build_log_time(frame.into())],
                &re_sdk_types::archetypes::TextLog::new(sentence.to_owned()),
            )
            .build()?;

        rec.send_chunk(chunk);
    }

    rec.flush_blocking()?;

    Ok(tmp_path)
}

pub fn create_recording_with_properties(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    user_defined_properties: BTreeMap<String, Vec<&dyn AsComponents>>,
) -> anyhow::Result<TempPath> {
    use re_chunk::Chunk;

    let tmp_path = {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(format!("{segment_id}.rrd"));
        TempPath::new(dir, path)
    };

    let rec = re_sdk::RecordingStreamBuilder::new("rerun_example_properties")
        .recording_id(segment_id)
        // NOTE: Don't send builtin properties (e.g. recording start time): these are non
        // deterministic (neither their values nor their Chunk/Row IDs) and are not what we're
        // trying to test anyhow. We'll be sending our own properties below.
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    for (prop_name, properties) in user_defined_properties {
        let property_path =
            re_log_types::EntityPath::properties().join(&re_log_types::EntityPath::from(prop_name));

        let mut chunk_builder = Chunk::builder_with_id(next_chunk_id(), property_path);

        for property in properties {
            chunk_builder =
                chunk_builder.with_archetype(next_row_id(), TimePoint::default(), property);
        }

        let chunk = chunk_builder.build()?;
        rec.send_chunk(chunk);
    }

    rec.flush_blocking()?;

    Ok(tmp_path)
}

/// Create a minimal rerun recording with one entity and one component.
///
/// Depending on the `is_binary` argument, the component will have underlying
/// arrow type of either `List[u8]` or `Binary`.
pub fn create_minimal_binary_recording_in(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    entity_path: &str,
    is_binary: bool,
    in_dir: &std::path::Path,
) -> anyhow::Result<PathBuf> {
    use re_chunk::Chunk;
    use re_log_types::{TimeInt, build_log_time};
    use re_sdk::{ComponentDescriptor, SerializedComponentBatch};

    if !std::fs::metadata(in_dir)?.is_dir() {
        return Err(anyhow::anyhow!("Expected `in_dir` to be a directory"));
    }

    let tmp_path = in_dir.join(format!("{segment_id}.rrd"));

    let rec = re_sdk::RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    let data: Vec<&[u8]> = vec![b"hello", b"rerun"];

    let array: ArrayRef = if is_binary {
        Arc::new(arrow::array::BinaryArray::from(data))
    } else {
        let list_array =
            arrow::array::ListArray::from_iter_primitive::<arrow::datatypes::UInt8Type, _, _>(
                data.iter()
                    .map(|slice| Some(slice.iter().copied().map(Some))),
            );
        Arc::new(list_array)
    };

    let frame = TimeInt::new_temporal(10);

    let chunk = Chunk::builder_with_id(next_chunk_id(), entity_path)
        .with_serialized_batch(
            next_row_id(),
            [build_log_time(frame.into())],
            SerializedComponentBatch::new(array, ComponentDescriptor::partial("data")),
        )
        .build()?;

    rec.send_chunk(chunk);

    rec.flush_blocking()?;

    Ok(tmp_path)
}

/// Creates a recording that can be split into multiple chunks.
///
/// This function creates an intentionally unsorted RRD. Each entity will have 9
/// rows of unsorted data. When combined with the environment variable
/// `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED=3` it will produce 3 chunks of 3 rows each.
/// The middle chunk will have nulls in some of the data.
pub fn multi_chunked_entities_recording(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    entity_paths: &[&str],
) -> anyhow::Result<TempPath> {
    use re_chunk::{Chunk, TimePoint};
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::{EntityPath, TimeInt, build_frame_nr};

    let tmp_dir = tempfile::tempdir()?;
    let in_dir = tmp_dir.path();

    if !std::fs::metadata(in_dir)?.is_dir() {
        return Err(anyhow::anyhow!("Expected `in_dir` to be a directory"));
    }

    let tmp_path = in_dir.join(format!("{segment_id}.rrd"));

    let rec = RecordingStreamBuilder::new(format!("rerun_example_{segment_id}"))
        .recording_id(segment_id)
        .send_properties(false)
        .save(tmp_path.clone())?;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    for base_time in [0, 30, 60] {
        for entity_path in entity_paths {
            let entity_path = EntityPath::from(*entity_path);

            // Sequential frames
            let frame1 = TimeInt::new_temporal(10 + base_time);
            let frame2 = TimeInt::new_temporal(20 + base_time);
            let frame3 = TimeInt::new_temporal(30 + base_time);

            // Data for each frame
            let points1 = MyPoint::from_iter(0..1);
            let points2 = MyPoint::from_iter(1..2);
            let points3 = MyPoint::from_iter(2..3);

            let colors1 = MyColor::from_iter(0..1);
            let colors2 = MyColor::from_iter(1..2);
            let colors3 = MyColor::from_iter(2..3);

            let labels = vec![MyLabel("simple".to_owned())];

            let mut component1 = vec![(MyPoints::descriptor_points(), Some(&points1 as _))];
            let mut component2 = vec![(MyPoints::descriptor_points(), Some(&points2 as _))];
            let mut component3 = vec![(MyPoints::descriptor_points(), Some(&points3 as _))];

            // Send nulls for the middle batch for the color component
            if base_time != 30 {
                component1.push((MyPoints::descriptor_colors(), Some(&colors1 as _)));
                component2.push((MyPoints::descriptor_colors(), Some(&colors2 as _)));
                component3.push((MyPoints::descriptor_colors(), Some(&colors3 as _)));
            }

            let chunk = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
                .with_sparse_component_batches(next_row_id(), [build_frame_nr(frame1)], component1)
                .with_sparse_component_batches(next_row_id(), [build_frame_nr(frame3)], component2)
                .with_sparse_component_batches(next_row_id(), [build_frame_nr(frame2)], component3)
                .build()?;

            rec.send_chunk(chunk);

            // send static data only once
            if base_time == 0 {
                let static_chunk = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
                    .with_sparse_component_batches(
                        next_row_id(),
                        TimePoint::default(),
                        [(MyPoints::descriptor_labels(), Some(&labels as _))],
                    )
                    .build()?;

                rec.send_chunk(static_chunk);
            }
        }
    }

    rec.flush_blocking()?;

    Ok(TempPath::new(tmp_dir, tmp_path))
}
