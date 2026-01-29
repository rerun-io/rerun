#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;

use itertools::Itertools as _;
use re_arrow_util::RecordBatchTestExt as _;
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_log_encoding::{
    Decodable as _, DecoderApp, Encoder, RrdManifest, RrdManifestBuilder, StreamFooter,
    StreamFooterEntry, ToApplication as _, ToTransport as _,
};
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{ArrowMsg, LogMsg, StoreId, StoreKind, build_log_time};
use re_protos::external::prost::Message as _;

#[test]
fn simple_manifest() {
    let rrd_manifest = {
        let mut builder = RrdManifestBuilder::default();
        let mut byte_offset_excluding_header = 0;
        for msg in generate_recording_chunks(1) {
            let chunk_batch = re_sorbet::ChunkBatch::try_from(&msg.batch).unwrap();

            let transport_uncompressed = msg
                .to_transport((
                    generate_recording_store_id(),
                    re_log_encoding::Compression::Off,
                ))
                .unwrap();
            let transport_compressed = msg
                .to_transport((
                    generate_recording_store_id(),
                    re_log_encoding::Compression::LZ4,
                ))
                .unwrap();

            let chunk_byte_size = transport_compressed.encoded_len() as u64;
            let chunk_byte_size_uncompressed = transport_uncompressed.encoded_len() as u64;

            let chunk_byte_span_excluding_header = re_span::Span {
                start: byte_offset_excluding_header,
                len: chunk_byte_size,
            };
            builder
                .append(
                    &chunk_batch,
                    chunk_byte_span_excluding_header,
                    chunk_byte_size_uncompressed,
                )
                .unwrap();

            byte_offset_excluding_header += chunk_byte_size;
        }

        builder.build(StoreId::empty_recording()).unwrap()
    };

    let rrd_manifest_batch = &rrd_manifest.data;

    let static_map = rrd_manifest
        .get_static_data_as_a_map()
        .unwrap()
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect::<BTreeMap<_, _>>()))
        .collect::<BTreeMap<_, _>>();

    let temporal_map = rrd_manifest
        .get_temporal_data_as_a_map()
        .unwrap()
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                v.into_iter()
                    .map(|(k, v)| (k, v.into_iter().collect::<BTreeMap<_, _>>()))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    insta::assert_snapshot!(
        "simple_manifest_batch_native_map_static",
        format!("{:#?}", static_map),
    );
    insta::assert_snapshot!(
        "simple_manifest_batch_native_map_temporal",
        format!("{:#?}", temporal_map),
    );

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
fn footer_roundtrip() {
    let msgs_expected_recording = generate_recording(generate_recording_chunks(1)).collect_vec();
    let msgs_expected_blueprint = generate_blueprint(generate_blueprint_chunks(2)).collect_vec();

    let msgs_encoded = Encoder::encode(
        msgs_expected_recording
            .clone()
            .into_iter()
            .map(Ok)
            .chain(msgs_expected_blueprint.clone().into_iter().map(Ok)),
    )
    .unwrap();

    let store_id_recording = generate_recording_store_id();
    let store_id_blueprint = generate_blueprint_store_id();

    let mut decoder = DecoderApp::decode_lazy(msgs_encoded.as_slice());
    let mut msgs_decoded_recording = Vec::new();
    let mut msgs_decoded_blueprint = Vec::new();
    for msg in &mut decoder {
        let msg = msg.unwrap();
        match msg {
            LogMsg::ArrowMsg(store_id, arrow_msg) => match store_id {
                id if id == store_id_recording => msgs_decoded_recording.push(arrow_msg),
                id if id == store_id_blueprint => msgs_decoded_blueprint.push(arrow_msg),
                _ => unreachable!(),
            },

            LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand(_) => {}
        }
    }

    let stream_footer_start = msgs_encoded
        .len()
        .checked_sub(re_log_encoding::StreamFooter::ENCODED_SIZE_BYTES)
        .unwrap();
    let stream_footer =
        re_log_encoding::StreamFooter::from_rrd_bytes(&msgs_encoded[stream_footer_start..])
            .unwrap();

    let StreamFooterEntry {
        rrd_footer_byte_span_from_start_excluding_header,
        crc_excluding_header,
    } = stream_footer.entries[0];

    let rrd_footer_range = rrd_footer_byte_span_from_start_excluding_header
        .try_cast::<usize>()
        .unwrap()
        .range();
    let rrd_footer_bytes = &msgs_encoded[rrd_footer_range];

    similar_asserts::assert_eq!(
        crc_excluding_header,
        StreamFooter::compute_crc(rrd_footer_bytes)
    );

    let rrd_footer =
        re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(rrd_footer_bytes).unwrap();
    let mut rrd_footer = rrd_footer.to_application(()).unwrap();

    let rrd_manifest_recording =
        RrdManifest::try_new(rrd_footer.manifests.remove(&store_id_recording).unwrap()).unwrap();
    let rrd_manifest_blueprint =
        RrdManifest::try_new(rrd_footer.manifests.remove(&store_id_blueprint).unwrap()).unwrap();

    fn decode_messages(msgs_encoded: &[u8], rrd_manifest: &RrdManifest) -> Vec<ArrowMsg> {
        itertools::izip!(
            rrd_manifest.col_chunk_byte_offset(),
            rrd_manifest.col_chunk_byte_size(),
        )
        .map(|(&offset, &size)| {
            let chunk_start_excluding_header = offset as usize;
            let chunk_end_excluding_header = chunk_start_excluding_header + size as usize;
            let buf = &msgs_encoded[chunk_start_excluding_header..chunk_end_excluding_header];
            let arrow_msg = re_protos::log_msg::v1alpha1::ArrowMsg::decode(buf).unwrap();
            arrow_msg.to_application(()).unwrap()
        })
        .collect()
    }

    let (msgs_decoded_recording_from_footer, msgs_decoded_blueprint_from_footer) = (
        decode_messages(&msgs_encoded, &rrd_manifest_recording),
        decode_messages(&msgs_encoded, &rrd_manifest_blueprint),
    );

    // Check that the RRD manifests decoded "traditionally" match those obtained via random access / footer.

    let sequential_manifests = decoder.rrd_manifests().unwrap();
    let rrd_manifest_blueprint_sequential = sequential_manifests
        .iter()
        .find(|m| m.store_id.kind() == StoreKind::Blueprint)
        .cloned()
        .unwrap();
    let rrd_manifest_recording_sequential = sequential_manifests
        .iter()
        .find(|m| m.store_id.kind() == StoreKind::Recording)
        .cloned()
        .unwrap();

    insta::assert_snapshot!(
        "rrd_manifest_blueprint",
        rrd_manifest_blueprint_sequential.data.format_snapshot(true),
    );
    insta::assert_snapshot!(
        "rrd_manifest_blueprint_schema",
        rrd_manifest_blueprint_sequential
            .data
            .format_schema_snapshot(),
    );
    insta::assert_snapshot!(
        "rrd_manifest_recording",
        rrd_manifest_recording_sequential.data.format_snapshot(true),
    );
    insta::assert_snapshot!(
        "rrd_manifest_recording_schema",
        rrd_manifest_recording_sequential
            .data
            .format_schema_snapshot(),
    );

    similar_asserts::assert_eq!(
        rrd_manifest_recording_sequential.data.format_snapshot(true),
        rrd_manifest_recording.data().format_snapshot(true),
        "RRD manifest decoded sequentially should be identical to the one decoded by jumping via the footer",
    );
    // Same test but check everything, not just the manifest data (we do both cause we want a nice diff for the manifest data)
    similar_asserts::assert_eq!(
        &rrd_manifest_recording_sequential,
        rrd_manifest_recording.raw(),
        "RRD manifest decoded sequentially should be identical to the one decoded by jumping via the footer",
    );

    similar_asserts::assert_eq!(
        rrd_manifest_blueprint_sequential.data.format_snapshot(true),
        rrd_manifest_blueprint.data().format_snapshot(true),
        "RRD manifest decoded sequentially should be identical to the one decoded by jumping via the footer",
    );
    // Same test but check everything, not just the manifest data (we do both cause we want a nice diff for the manifest data)
    similar_asserts::assert_eq!(
        &rrd_manifest_blueprint_sequential,
        rrd_manifest_blueprint.raw(),
        "RRD manifest decoded sequentially should be identical to the one decoded by jumping via the footer",
    );

    // Check that the data decoded "traditionally" matches the data decoded via random access / footer.

    similar_asserts::assert_eq!(
        msgs_decoded_recording,
        msgs_decoded_recording_from_footer,
        "chunks decoded sequentially should be identical to those decoded by jumping around using the RRD manifest in the footer",
    );

    similar_asserts::assert_eq!(
        msgs_decoded_blueprint,
        msgs_decoded_blueprint_from_footer,
        "chunks decoded sequentially should be identical to those decoded by jumping around using the RRD manifest in the footer",
    );

    let msgs_reencoded = Encoder::encode(
        itertools::chain!(
            generate_recording(msgs_decoded_recording_from_footer.into_iter()),
            generate_blueprint(msgs_decoded_blueprint_from_footer.into_iter())
        )
        .map(Ok),
    )
    .unwrap();

    // And finally, let's reencode all the messages we decoded back into an RRD stream
    {
        let reencoded_stream_footer_start = msgs_reencoded
            .len()
            .checked_sub(re_log_encoding::StreamFooter::ENCODED_SIZE_BYTES)
            .unwrap();
        let reencoded_stream_footer = re_log_encoding::StreamFooter::from_rrd_bytes(
            &msgs_reencoded[reencoded_stream_footer_start..],
        )
        .unwrap();

        let StreamFooterEntry {
            rrd_footer_byte_span_from_start_excluding_header,
            crc_excluding_header,
        } = reencoded_stream_footer.entries[0];

        let reencoded_rrd_footer_range = rrd_footer_byte_span_from_start_excluding_header
            .try_cast::<usize>()
            .unwrap()
            .range();
        let reencoded_rrd_footer_bytes = &msgs_reencoded[reencoded_rrd_footer_range];

        similar_asserts::assert_eq!(
            crc_excluding_header,
            StreamFooter::compute_crc(reencoded_rrd_footer_bytes)
        );

        let reencoded_rrd_footer =
            re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(reencoded_rrd_footer_bytes)
                .unwrap();
        let mut reencoded_rrd_footer = reencoded_rrd_footer.to_application(()).unwrap();

        let reencoded_rrd_manifest_recording = RrdManifest::try_new(
            reencoded_rrd_footer
                .manifests
                .remove(&store_id_recording)
                .unwrap(),
        )
        .unwrap();
        let reencoded_rrd_manifest_blueprint = RrdManifest::try_new(
            reencoded_rrd_footer
                .manifests
                .remove(&store_id_blueprint)
                .unwrap(),
        )
        .unwrap();

        similar_asserts::assert_eq!(
            rrd_manifest_recording.data().format_snapshot(true),
            reencoded_rrd_manifest_recording
                .data()
                .format_snapshot(true),
            "Reencoded RRD manifest should be identical to the original one",
        );
        // Same test but check everything, not just the manifest data (we do both cause we want a nice diff for the manifest data)
        similar_asserts::assert_eq!(
            &rrd_manifest_recording,
            &reencoded_rrd_manifest_recording,
            "Reencoded RRD manifest should be identical to the original one",
        );

        similar_asserts::assert_eq!(
            rrd_manifest_blueprint.data().format_snapshot(true),
            reencoded_rrd_manifest_blueprint
                .data()
                .format_snapshot(true),
            "Reencoded RRD manifest should be identical to the original one",
        );
        // Same test but check everything, not just the manifest data (we do both cause we want a nice diff for the manifest data)
        similar_asserts::assert_eq!(
            &rrd_manifest_blueprint,
            &reencoded_rrd_manifest_blueprint,
            "Reencoded RRD manifest should be identical to the original one",
        );
    }
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

    assert_eq!(
        1,
        stream_footer.entries.len(),
        "Stream footers always point to exactly 1 RRD footer at the moment"
    );

    let StreamFooterEntry {
        rrd_footer_byte_span_from_start_excluding_header,
        crc_excluding_header,
    } = stream_footer.entries[0];

    let rrd_footer_range = rrd_footer_byte_span_from_start_excluding_header
        .try_cast::<usize>()
        .unwrap()
        .range();
    let rrd_footer_bytes = &msgs_encoded[rrd_footer_range];

    similar_asserts::assert_eq!(
        crc_excluding_header,
        StreamFooter::compute_crc(rrd_footer_bytes)
    );

    let rrd_footer =
        re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(rrd_footer_bytes).unwrap();
    let rrd_footer = rrd_footer.to_application(()).unwrap();

    // Right now, the implemented behavior is that we end up with an empty footer, i.e. there are
    // no manifests in it.
    // Whether that's the correct behavior is another question, but at least it is defined for now
    // and can be changed.
    assert!(rrd_footer.manifests.is_empty());
}

// ---

fn generate_recording_store_id() -> StoreId {
    StoreId::recording("my_app", "my_recording")
}

fn generate_recording(
    chunks: impl Iterator<Item = re_log_types::ArrowMsg>,
) -> impl Iterator<Item = LogMsg> {
    let store_id = generate_recording_store_id();

    std::iter::once(LogMsg::SetStoreInfo(re_log_types::SetStoreInfo {
        row_id: *RowId::ZERO,
        info: re_log_types::StoreInfo {
            store_id: store_id.clone(),
            cloned_from: None,
            store_source: re_log_types::StoreSource::Unknown,
            store_version: Some(re_build_info::CrateVersion::new(1, 2, 3)),
        },
    }))
    .chain(chunks.map(move |chunk| LogMsg::ArrowMsg(store_id.clone(), chunk)))
}

fn generate_recording_chunks(tuid_prefix: u64) -> impl Iterator<Item = re_log_types::ArrowMsg> {
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::{TimeInt, TimeType, Timeline, build_frame_nr};

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    let entity_path1 = "my_entity1";
    let entity_path2 = "my_entity2";

    fn build_elapsed(value: i64) -> (Timeline, TimeInt) {
        (
            // Intentionally bringing some whitespaces into the mix 🫠
            Timeline::new("elapsed time", TimeType::DurationNs),
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

            Chunk::builder_with_id(next_chunk_id(), entity_path1)
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
            let labels_static = vec![MyLabel("static".to_owned())];
            // It is super important that we test what happens when a single entity+component pair
            // ends up with both static and temporal data, something that the viewer has always
            // been able to ingest!
            let colors_static = MyColor::from_iter(66..67);

            Chunk::builder_with_id(next_chunk_id(), entity_path1)
                .with_sparse_component_batches(
                    next_row_id(),
                    TimePoint::default(),
                    [
                        (MyPoints::descriptor_labels(), Some(&labels_static as _)), //
                        (MyPoints::descriptor_colors(), Some(&colors_static as _)), //
                    ],
                )
                .build()
                .unwrap()
                .to_arrow_msg()
                .unwrap()
        },
        // Just testing with more than 1 entity.
        {
            let points_static = MyPoint::from_iter(42..43);
            let colors_static = MyColor::from_iter(66..67);
            let labels_static = vec![MyLabel("static".to_owned())];

            Chunk::builder_with_id(next_chunk_id(), entity_path2)
                .with_sparse_component_batches(
                    next_row_id(),
                    TimePoint::default(),
                    [
                        (MyPoints::descriptor_points(), Some(&points_static as _)), //
                        (MyPoints::descriptor_colors(), Some(&colors_static as _)), //
                        (MyPoints::descriptor_labels(), Some(&labels_static as _)), //
                    ],
                )
                .build()
                .unwrap()
                .to_arrow_msg()
                .unwrap()
        },
    ]
    .into_iter()
}

fn generate_blueprint_store_id() -> StoreId {
    StoreId::new(StoreKind::Blueprint, "my_app", "my_blueprint")
}

fn generate_blueprint(
    chunks: impl Iterator<Item = re_log_types::ArrowMsg>,
) -> impl Iterator<Item = LogMsg> {
    let store_id = generate_blueprint_store_id();

    std::iter::once(LogMsg::SetStoreInfo(re_log_types::SetStoreInfo {
        row_id: *RowId::ZERO,
        info: re_log_types::StoreInfo {
            store_id: store_id.clone(),
            cloned_from: None,
            store_source: re_log_types::StoreSource::Unknown,
            store_version: Some(re_build_info::CrateVersion::new(4, 5, 6)),
        },
    }))
    .chain(chunks.map(move |chunk| LogMsg::ArrowMsg(store_id.clone(), chunk)))
}

fn generate_blueprint_chunks(tuid_prefix: u64) -> impl Iterator<Item = re_log_types::ArrowMsg> {
    use re_log_types::{EntityPath, TimeInt, build_frame_nr};
    use re_sdk_types::blueprint::archetypes::TimePanelBlueprint;

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    [
        Chunk::builder_with_id(next_chunk_id(), EntityPath::from("/time_panel"))
            .with_archetype(
                next_row_id(),
                [build_frame_nr(TimeInt::new_temporal(0))],
                &TimePanelBlueprint::default().with_fps(60.0),
            )
            .build()
            .unwrap()
            .to_arrow_msg()
            .unwrap(),
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
