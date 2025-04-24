use rerun::{
    external::{
        anyhow,
        re_log_types::{
            build_frame_nr, build_log_time,
            example_components::{MyColor, MyLabel, MyPoint},
        },
    },
    log::{Chunk, ChunkId, RowId},
    time::{TimeInt, TimeType},
    Component as _, EntityPath, RecordingStream, TimePoint, Timeline,
};

fn next_chunk_id_generator() -> impl FnMut() -> ChunkId {
    let mut chunk_id = ChunkId::ZERO;
    move || {
        chunk_id = chunk_id.next();
        chunk_id
    }
}

fn next_row_id_generator() -> impl FnMut() -> RowId {
    let mut row_id = RowId::ZERO;
    move || {
        row_id = row_id.next();
        row_id
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    rerun::external::re_log::setup_logging();

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_determinism")
        .recording_id("some_reproducible_id")
        .send_properties(false)
        .stdout()?;

    create_nasty_recording(&rec, &["entity_a"])?;

    Ok(())
}

pub fn create_nasty_recording(rec: &RecordingStream, entity_paths: &[&str]) -> anyhow::Result<()> {
    let mut next_chunk_id = next_chunk_id_generator();
    let mut next_row_id = next_row_id_generator();

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
                    (MyPoint::descriptor(), Some(&points1 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), Some(&labels1 as _)), // shadowed by static
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
                    (MyPoint::descriptor(), Some(&points3 as _)),
                    (MyColor::descriptor(), Some(&colors3 as _)),
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
                    (MyPoint::descriptor(), Some(&points5 as _)),
                    (MyColor::descriptor(), None),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoint::descriptor(), Some(&points7_1 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoint::descriptor(), Some(&points7_2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoint::descriptor(), Some(&points7_3 as _))],
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
                [(MyPoint::descriptor(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame3)],
                [
                    (MyPoint::descriptor(), Some(&points3 as _)),
                    (MyColor::descriptor(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoint::descriptor(), Some(&points4 as _))],
            )
            .build()?;

        rec.send_chunk(chunk2);

        let chunk3 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame2)],
                [(MyPoint::descriptor(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoint::descriptor(), Some(&points4 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame6)],
                [(MyPoint::descriptor(), Some(&points6 as _))],
            )
            .build()?;

        rec.send_chunk(chunk3);

        let chunk4 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyColor::descriptor(), Some(&colors4 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame5)],
                [(MyColor::descriptor(), Some(&colors5 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame7)],
                [(MyColor::descriptor(), Some(&colors7 as _))],
            )
            .build()?;

        rec.send_chunk(chunk4);

        let chunk5 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyLabel::descriptor(), Some(&labels2 as _))],
            )
            .build()?;

        rec.send_chunk(chunk5);

        let chunk6 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyLabel::descriptor(), Some(&labels3 as _))],
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
                [(MyColor::descriptor(), Some(&colors99 as _))],
            )
            .build()?;

        rec.send_chunk(chunk7);

        let chunk8 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame95)],
                [(MyLabel::descriptor(), Some(&labels95 as _))],
            )
            .build()?;

        rec.send_chunk(chunk8);
    }

    rec.flush_blocking();

    Ok(())
}
