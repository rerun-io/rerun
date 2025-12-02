#![expect(clippy::unwrap_used)]

use re_arrow_util::RecordBatchTestExt as _;
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_log_encoding::{Decodable as _, Encoder, RrdManifestBuilder, ToApplication as _};
use re_log_types::{LogMsg, StoreId, build_log_time, external::re_tuid::Tuid};

#[test]
fn simple_manifest() {
    let rrd_manifest_batch = {
        let mut builder = RrdManifestBuilder::default();
        let mut byte_offset_excluding_header = 0;
        for msg in generate_recording_chunks(1) {
            let chunk_batch = re_sorbet::ChunkBatch::try_from(&msg.batch).unwrap();
            let chunk_byte_size = chunk_batch.heap_size_bytes().unwrap();

            builder
                .append(&chunk_batch, byte_offset_excluding_header, chunk_byte_size)
                .unwrap();

            byte_offset_excluding_header += chunk_byte_size;
        }
        builder.into_record_batch().unwrap()
    };

    insta::assert_snapshot!(
        "simple_manifest_batch",
        rrd_manifest_batch.format_snapshot(true),
    );
    insta::assert_snapshot!(
        "simple_manifest_batch_schema",
        rrd_manifest_batch.format_schema_snapshot(),
    );
}

#[test]
fn footer_empty() {
    fn generate_store_id() -> StoreId {
        StoreId::recording("my_app", "my_empty_recording")
    }

    fn generate_recording() -> impl Iterator<Item = LogMsg> {
        let store_id = generate_store_id();

        std::iter::once(LogMsg::SetStoreInfo(re_log_types::SetStoreInfo {
            row_id: *RowId::ZERO,
            info: re_log_types::StoreInfo {
                store_id: store_id.clone(),
                cloned_from: None,
                store_source: re_log_types::StoreSource::Unknown,
                store_version: Some(re_build_info::CrateVersion::new(1, 2, 3)),
                is_partial: false,
            },
        }))
    }

    let msgs_encoded = Encoder::encode(generate_recording().map(Ok)).unwrap();

    let stream_footer_start = msgs_encoded
        .len()
        .checked_sub(re_log_encoding::StreamFooter::ENCODED_SIZE_BYTES)
        .unwrap();
    let stream_footer =
        re_log_encoding::StreamFooter::from_rrd_bytes(&msgs_encoded[stream_footer_start..])
            .unwrap();

    let rrd_footer_range = stream_footer
        .rrd_footer_byte_span_from_start_excluding_header
        .try_cast::<usize>()
        .unwrap()
        .range();
    let rrd_footer_bytes = &msgs_encoded[rrd_footer_range];

    {
        let crc = re_log_encoding::StreamFooter::from_rrd_footer_bytes(
            stream_footer
                .rrd_footer_byte_span_from_start_excluding_header
                .start,
            rrd_footer_bytes,
        )
        .crc_excluding_header;
        similar_asserts::assert_eq!(stream_footer.crc_excluding_header, crc);
    }

    let rrd_footer =
        re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(rrd_footer_bytes).unwrap();
    let _rrd_footer = rrd_footer.to_application(()).unwrap();
}

// ---

fn generate_recording_chunks(tuid_prefix: u64) -> impl Iterator<Item = re_log_types::ArrowMsg> {
    use re_log_types::{
        TimeInt, TimeType, Timeline, build_frame_nr,
        example_components::{MyColor, MyLabel, MyPoint, MyPoints},
    };

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    let entity_path = "my_entity";

    fn build_elapsed(value: i64) -> (Timeline, TimeInt) {
        (
            Timeline::new("elapsed", TimeType::DurationNs),
            TimeInt::saturated_temporal(value * 1e9 as i64),
        )
    }

    fn build_timepoint(time: TimeInt) -> [(Timeline, TimeInt); 3] {
        [
            build_frame_nr(time),
            build_log_time(time.into()),
            build_elapsed(time.as_i64()),
        ]
    }

    [
        {
            let frame1 = TimeInt::new_temporal(10);
            let frame2 = TimeInt::new_temporal(20);
            let frame3 = TimeInt::new_temporal(30);
            let frame4 = TimeInt::new_temporal(40);

            let points1 = MyPoint::from_iter(0..1);
            let points3 = MyPoint::from_iter(2..3);
            let points4 = MyPoint::from_iter(3..4);

            let colors2 = MyColor::from_iter(1..2);
            let colors3 = MyColor::from_iter(2..3);

            Chunk::builder_with_id(next_chunk_id(), entity_path)
                .with_sparse_component_batches(
                    next_row_id(),
                    build_timepoint(frame1),
                    [(MyPoints::descriptor_points(), Some(&points1 as _))],
                )
                .with_sparse_component_batches(
                    next_row_id(),
                    build_timepoint(frame2),
                    [(MyPoints::descriptor_colors(), Some(&colors2 as _))],
                )
                .with_sparse_component_batches(
                    next_row_id(),
                    build_timepoint(frame3),
                    [
                        (MyPoints::descriptor_points(), Some(&points3 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    next_row_id(),
                    build_timepoint(frame4),
                    [(MyPoints::descriptor_points(), Some(&points4 as _))],
                )
                .build()
                .unwrap()
                .to_arrow_msg()
                .unwrap()
        },
        {
            let labels = vec![MyLabel("simple".to_owned())];

            Chunk::builder_with_id(next_chunk_id(), entity_path)
                .with_sparse_component_batches(
                    next_row_id(),
                    TimePoint::default(),
                    [(MyPoints::descriptor_labels(), Some(&labels as _))],
                )
                .build()
                .unwrap()
                .to_arrow_msg()
                .unwrap()
        },
    ]
    .into_iter()
}

fn next_chunk_id_generator(prefix: u64) -> impl FnMut() -> ChunkId {
    let mut chunk_id = ChunkId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
    move || {
        chunk_id = chunk_id.next();
        chunk_id
    }
}

fn next_row_id_generator(prefix: u64) -> impl FnMut() -> RowId {
    let mut row_id = RowId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
    move || {
        row_id = row_id.next();
        row_id
    }
}
