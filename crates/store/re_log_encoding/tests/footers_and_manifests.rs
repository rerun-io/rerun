#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{Array as _, BinaryArray, RecordBatch};
use arrow::datatypes::Field;
use itertools::Itertools as _;
use re_arrow_util::RecordBatchTestExt as _;
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_log_encoding::{
    Decodable as _, DecoderApp, Encoder, RawRrdManifest, RrdManifest, RrdManifestBuilder,
    StreamFooter, StreamFooterEntry, ToApplication as _, ToTransport as _,
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
        .calc_static_map()
        .unwrap()
        .clone()
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect::<BTreeMap<_, _>>()))
        .collect::<BTreeMap<_, _>>();

    let temporal_map = rrd_manifest
        .calc_temporal_map()
        .unwrap()
        .clone()
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
        format!("{static_map:#?}"),
    );
    insta::assert_snapshot!(
        "simple_manifest_batch_native_map_temporal",
        format!("{temporal_map:#?}"),
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

    let raw_manifest_recording = rrd_footer.manifests.remove(&store_id_recording).unwrap();
    let raw_manifest_blueprint = rrd_footer.manifests.remove(&store_id_blueprint).unwrap();
    let rrd_manifest_recording = RrdManifest::try_new(&raw_manifest_recording).unwrap();
    let rrd_manifest_blueprint = RrdManifest::try_new(&raw_manifest_blueprint).unwrap();

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

    // Note: we compare semantic fields rather than raw data because `RrdManifest::try_new`
    // prunes sparse columns from the RecordBatch for memory efficiency. The sequential decoder
    // returns the full unpruned `RawRrdManifest`, so raw data comparison would fail.
    similar_asserts::assert_eq!(
        rrd_manifest_recording_sequential.store_id,
        raw_manifest_recording.store_id,
        "RRD manifest decoded sequentially should have the same store_id as the one decoded via the footer",
    );
    similar_asserts::assert_eq!(
        rrd_manifest_recording_sequential.sorbet_schema,
        *rrd_manifest_recording.sorbet_schema(),
        "RRD manifest decoded sequentially should have the same sorbet_schema as the one decoded via the footer",
    );
    similar_asserts::assert_eq!(
        rrd_manifest_recording_sequential.sorbet_schema_sha256,
        raw_manifest_recording.sorbet_schema_sha256,
        "RRD manifest decoded sequentially should have the same sorbet_schema_sha256 as the one decoded via the footer",
    );

    similar_asserts::assert_eq!(
        rrd_manifest_blueprint_sequential.store_id,
        raw_manifest_blueprint.store_id,
        "RRD manifest decoded sequentially should have the same store_id as the one decoded via the footer",
    );
    similar_asserts::assert_eq!(
        rrd_manifest_blueprint_sequential.sorbet_schema,
        *rrd_manifest_blueprint.sorbet_schema(),
        "RRD manifest decoded sequentially should have the same sorbet_schema as the one decoded via the footer",
    );
    similar_asserts::assert_eq!(
        rrd_manifest_blueprint_sequential.sorbet_schema_sha256,
        raw_manifest_blueprint.sorbet_schema_sha256,
        "RRD manifest decoded sequentially should have the same sorbet_schema_sha256 as the one decoded via the footer",
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

        let reencoded_raw_recording = reencoded_rrd_footer
            .manifests
            .remove(&store_id_recording)
            .unwrap();
        let reencoded_raw_blueprint = reencoded_rrd_footer
            .manifests
            .remove(&store_id_blueprint)
            .unwrap();
        let reencoded_rrd_manifest_recording =
            RrdManifest::try_new(&reencoded_raw_recording).unwrap();
        let reencoded_rrd_manifest_blueprint =
            RrdManifest::try_new(&reencoded_raw_blueprint).unwrap();

        similar_asserts::assert_eq!(
            rrd_manifest_recording
                .chunk_fetcher_rb()
                .format_snapshot(true),
            reencoded_rrd_manifest_recording
                .chunk_fetcher_rb()
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
            rrd_manifest_blueprint
                .chunk_fetcher_rb()
                .format_snapshot(true),
            reencoded_rrd_manifest_blueprint
                .chunk_fetcher_rb()
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

/// Helper: add a `chunk_key` column to a `RawRrdManifest`, returning a new manifest.
fn add_chunk_keys_to_raw(raw: &RawRrdManifest) -> RawRrdManifest {
    let num_rows = raw.data.num_rows();
    let keys: Vec<Vec<u8>> = (0..num_rows)
        .map(|i| format!("key_{i}").into_bytes())
        .collect();
    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
    let chunk_key_array = BinaryArray::from_vec(key_refs);

    let schema = raw.data.schema();
    let mut fields: Vec<_> = schema.fields().iter().cloned().collect();
    let mut columns: Vec<_> = raw.data.columns().to_vec();

    fields.push(Arc::new(Field::new(
        RawRrdManifest::FIELD_CHUNK_KEY,
        arrow::datatypes::DataType::Binary,
        true,
    )));
    columns.push(Arc::new(chunk_key_array));

    let new_schema = Arc::new(arrow::datatypes::Schema::new_with_metadata(
        fields,
        schema.metadata().clone(),
    ));
    let num_rows = raw.data.num_rows();
    let new_batch = RecordBatch::try_new_with_options(
        new_schema,
        columns,
        &arrow::array::RecordBatchOptions::new().with_row_count(Some(num_rows)),
    )
    .unwrap();

    RawRrdManifest {
        store_id: raw.store_id.clone(),
        sorbet_schema: raw.sorbet_schema.clone(),
        sorbet_schema_sha256: raw.sorbet_schema_sha256,
        data: new_batch,
    }
}

/// Verifies that concatenating manifests where some have `chunk_keys` and others don't
/// produces a correctly aligned result (null keys for manifests without them).
#[test]
fn concat_with_mixed_chunk_keys() {
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{TimeInt, build_frame_nr};

    let store_id = generate_recording_store_id();

    let mut next_chunk_id = next_chunk_id_generator(200);
    let mut next_row_id = next_row_id_generator(200);

    let mut make_chunk = |entity: &str, frame: i64| -> Chunk {
        let points = MyPoint::from_iter(0..1);
        let timepoint = TimePoint::from([build_frame_nr(TimeInt::new_temporal(frame))]);
        Chunk::builder_with_id(next_chunk_id(), entity)
            .with_sparse_component_batches(
                next_row_id(),
                timepoint,
                [(MyPoints::descriptor_points(), Some(&points as _))],
            )
            .build()
            .unwrap()
    };

    let chunks1 = [make_chunk("entity_a", 10), make_chunk("entity_a", 20)];
    let chunks2 = [make_chunk("entity_a", 30), make_chunk("entity_a", 40)];

    let raw1 =
        RawRrdManifest::build_in_memory_from_chunks(store_id.clone(), chunks1.iter()).unwrap();
    let raw2 =
        RawRrdManifest::build_in_memory_from_chunks(store_id.clone(), chunks2.iter()).unwrap();

    // raw1 gets chunk_keys, raw2 does not
    let raw1_with_keys = add_chunk_keys_to_raw(&raw1);

    let m1 = RrdManifest::try_new(&raw1_with_keys).unwrap();
    let m2 = RrdManifest::try_new(&raw2).unwrap();

    assert!(m1.col_chunk_key_raw().is_some());
    assert!(m2.col_chunk_key_raw().is_none());

    // Concat should handle mixed chunk_keys gracefully
    let combined = RrdManifest::concat(&[&m1, &m2]).unwrap();

    // Total chunks must equal sum of parts
    assert_eq!(combined.num_chunks(), 4);

    // chunk_keys should be present and aligned with the total number of chunks
    let combined_keys = combined
        .col_chunk_key_raw()
        .expect("combined manifest should have chunk_keys when any part has them");
    assert_eq!(
        combined_keys.len(),
        4,
        "chunk_keys array must have one entry per chunk"
    );

    // First two entries (from m1) should be non-null
    assert!(!combined_keys.is_null(0));
    assert!(!combined_keys.is_null(1));
    // Last two entries (from m2, which had no keys) should be null
    assert!(combined_keys.is_null(2));
    assert!(combined_keys.is_null(3));
}

/// Verifies that `heap_size_bytes` accounts for pre-extracted arrays that are NOT
/// in the pruned `chunk_fetcher_rb`.
#[test]
fn size_bytes_accounts_for_extracted_arrays() {
    use re_chunk::external::re_byte_size::SizeBytes as _;
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{TimeInt, build_frame_nr};

    let store_id = generate_recording_store_id();

    let mut next_chunk_id = next_chunk_id_generator(300);
    let mut next_row_id = next_row_id_generator(300);

    let mut make_chunk = |entity: &str, frame: i64| -> Chunk {
        let points = MyPoint::from_iter(0..1);
        let timepoint = TimePoint::from([build_frame_nr(TimeInt::new_temporal(frame))]);
        Chunk::builder_with_id(next_chunk_id(), entity)
            .with_sparse_component_batches(
                next_row_id(),
                timepoint,
                [(MyPoints::descriptor_points(), Some(&points as _))],
            )
            .build()
            .unwrap()
    };

    let chunks: Vec<_> = (0..10).map(|i| make_chunk("entity_a", i * 10)).collect();
    let manifest = RrdManifest::build_in_memory_from_chunks(store_id, chunks.iter()).unwrap();

    // Call heap_size_bytes on RrdManifest directly, not through Arc (which adds struct size overhead).
    let total_size =
        re_chunk::external::re_byte_size::SizeBytes::heap_size_bytes(manifest.as_ref());

    // chunk_entity_paths, chunk_num_rows, chunk_byte_sizes, chunk_byte_sizes_uncompressed
    // are NOT in the pruned chunk_fetcher_rb. They hold their own Arrow buffer allocations
    // (not shared with the pruned batch) and must be counted in heap_size_bytes.
    //
    // Compute what the pruned batch + maps alone would give us, then verify the total
    // is strictly larger — meaning the extracted arrays are actually being counted.
    let pruned_batch_and_maps_only = manifest.chunk_fetcher_rb().heap_size_bytes()
        + manifest.static_map().heap_size_bytes()
        + manifest.temporal_map().heap_size_bytes();

    assert!(
        total_size > pruned_batch_and_maps_only,
        "heap_size_bytes ({total_size}) must be strictly greater than the pruned batch + maps \
         ({pruned_batch_and_maps_only}). The extracted chunk_entity_paths, chunk_num_rows, \
         chunk_byte_sizes, and chunk_byte_sizes_uncompressed hold their own allocations and \
         must be counted."
    );
}

/// Verifies that `RawRrdManifest::concat` → `RrdManifest::try_new` produces the same result
/// as `RrdManifest::try_new` on each part → `RrdManifest::concat`.
#[test]
fn concat_raw_then_validate_vs_validate_then_concat() {
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{TimeInt, build_frame_nr};

    let store_id = generate_recording_store_id();

    let mut next_chunk_id = next_chunk_id_generator(100);
    let mut next_row_id = next_row_id_generator(100);

    // Helper: build a chunk with points and colors, either temporal or static.
    let mut make_chunk = |entity: &str, frame: Option<i64>| -> Chunk {
        let points = MyPoint::from_iter(0..1);
        let colors = MyColor::from_iter(0..1);
        let timepoint = match frame {
            Some(f) => TimePoint::from([build_frame_nr(TimeInt::new_temporal(f))]),
            None => TimePoint::default(),
        };
        Chunk::builder_with_id(next_chunk_id(), entity)
            .with_sparse_component_batches(
                next_row_id(),
                timepoint,
                [
                    (MyPoints::descriptor_points(), Some(&points as _)),
                    (MyPoints::descriptor_colors(), Some(&colors as _)),
                ],
            )
            .build()
            .unwrap()
    };

    // Three groups of chunks. Each group has the same component/timeline structure
    // so that the sorbet schemas match across manifests.
    let chunks1 = [
        make_chunk("entity_a", Some(10)),
        make_chunk("entity_a", Some(20)),
        make_chunk("entity_a", None),
    ];
    let chunks2 = [
        make_chunk("entity_a", Some(30)),
        make_chunk("entity_a", Some(40)),
        make_chunk("entity_a", None),
    ];
    let chunks3 = [
        make_chunk("entity_a", Some(50)),
        make_chunk("entity_a", Some(60)),
        make_chunk("entity_a", None),
    ];

    let raw1 =
        RawRrdManifest::build_in_memory_from_chunks(store_id.clone(), chunks1.iter()).unwrap();
    let raw2 =
        RawRrdManifest::build_in_memory_from_chunks(store_id.clone(), chunks2.iter()).unwrap();
    let raw3 =
        RawRrdManifest::build_in_memory_from_chunks(store_id.clone(), chunks3.iter()).unwrap();

    // Path A: concat raw manifests first, then validate into RrdManifest.
    let raw_concatenated = RawRrdManifest::concat(&[&raw1, &raw2, &raw3]).unwrap();
    let path_a = RrdManifest::try_new(&raw_concatenated).unwrap();

    // Path B: validate each raw manifest into RrdManifest first, then concat.
    let m1 = RrdManifest::try_new(&raw1).unwrap();
    let m2 = RrdManifest::try_new(&raw2).unwrap();
    let m3 = RrdManifest::try_new(&raw3).unwrap();
    let path_b = RrdManifest::concat(&[&m1, &m2, &m3]).unwrap();

    // Both paths must produce identical results.
    assert_eq!(path_a.num_chunks(), path_b.num_chunks(), "num_chunks");

    similar_asserts::assert_eq!(path_a.col_chunk_ids(), path_b.col_chunk_ids());
    similar_asserts::assert_eq!(
        path_a.col_chunk_entity_path().collect::<Vec<_>>(),
        path_b.col_chunk_entity_path().collect::<Vec<_>>(),
    );
    similar_asserts::assert_eq!(
        path_a.col_chunk_is_static().collect::<Vec<_>>(),
        path_b.col_chunk_is_static().collect::<Vec<_>>(),
    );
    similar_asserts::assert_eq!(path_a.col_chunk_num_rows(), path_b.col_chunk_num_rows());
    similar_asserts::assert_eq!(
        path_a.col_chunk_byte_offset(),
        path_b.col_chunk_byte_offset(),
    );
    similar_asserts::assert_eq!(path_a.col_chunk_byte_size(), path_b.col_chunk_byte_size());
    similar_asserts::assert_eq!(
        path_a.col_chunk_byte_size_uncompressed(),
        path_b.col_chunk_byte_size_uncompressed(),
    );

    assert_eq!(path_a.static_map(), path_b.static_map(), "static_map");
    assert_eq!(path_a.temporal_map(), path_b.temporal_map(), "temporal_map");

    similar_asserts::assert_eq!(path_a.recording_schema(), path_b.recording_schema());
    similar_asserts::assert_eq!(path_a.sorbet_schema(), path_b.sorbet_schema());
}
