#![expect(clippy::unwrap_used)]

use itertools::Itertools as _;

use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_log_encoding::{Decodable as _, DecoderApp, Encoder, RrdManifest, ToApplication as _};
use re_log_types::{ArrowMsg, LogMsg, StoreId, StoreKind, external::re_tuid::Tuid};
use re_protos::external::prost::Message as _;

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

    let rrd_footer_start =
        stream_footer.rrd_footer_byte_offset_from_start_excluding_header as usize;
    let rrd_footer_end = rrd_footer_start
        .checked_add(stream_footer.rrd_footer_byte_size_excluding_header as usize)
        .unwrap();
    let rrd_footer_bytes = &msgs_encoded[rrd_footer_start..rrd_footer_end];

    {
        let crc = re_log_encoding::StreamFooter::from_rrd_footer_bytes(
            rrd_footer_start as u64,
            rrd_footer_bytes,
        )
        .crc_excluding_header;
        similar_asserts::assert_eq!(stream_footer.crc_excluding_header, crc);
    }

    let rrd_footer =
        re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(rrd_footer_bytes).unwrap();
    let mut rrd_footer = rrd_footer.to_application(()).unwrap();

    let rrd_manifest_recording = rrd_footer.manifests.remove(&store_id_recording).unwrap();
    let rrd_manifest_blueprint = rrd_footer.manifests.remove(&store_id_blueprint).unwrap();

    fn decode_messages(msgs_encoded: &[u8], rrd_manifest: &RrdManifest) -> Vec<ArrowMsg> {
        itertools::izip!(
            rrd_manifest.col_chunk_byte_offset().unwrap(),
            rrd_manifest.col_chunk_byte_len().unwrap()
        )
        .map(|(offset, size)| {
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
        batch_to_comparable_string(&rrd_manifest_blueprint_sequential.data),
    );
    insta::assert_snapshot!(
        "rrd_manifest_blueprint_schema",
        schema_to_comparable_string(&rrd_manifest_blueprint_sequential.data.schema()),
    );
    insta::assert_snapshot!(
        "rrd_manifest_recording",
        batch_to_comparable_string(&rrd_manifest_recording_sequential.data),
    );
    insta::assert_snapshot!(
        "rrd_manifest_recording_schema",
        schema_to_comparable_string(&rrd_manifest_recording_sequential.data.schema()),
    );

    similar_asserts::assert_eq!(
        batch_to_comparable_string(&rrd_manifest_recording_sequential.data),
        batch_to_comparable_string(&rrd_manifest_recording.data),
        "RRD manifest decoded sequentially should be identical to the one decoded by jumping via the footer",
    );
    // Same test but check everything, not just the manifest data (we do both cause we want a nice diff for the manifest data)
    similar_asserts::assert_eq!(
        &rrd_manifest_recording_sequential,
        &rrd_manifest_recording,
        "RRD manifest decoded sequentially should be identical to the one decoded by jumping via the footer",
    );

    similar_asserts::assert_eq!(
        batch_to_comparable_string(&rrd_manifest_blueprint_sequential.data),
        batch_to_comparable_string(&rrd_manifest_blueprint.data),
        "RRD manifest decoded sequentially should be identical to the one decoded by jumping via the footer",
    );
    // Same test but check everything, not just the manifest data (we do both cause we want a nice diff for the manifest data)
    similar_asserts::assert_eq!(
        &rrd_manifest_blueprint_sequential,
        &rrd_manifest_blueprint,
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

        let reencoded_rrd_footer_start =
            reencoded_stream_footer.rrd_footer_byte_offset_from_start_excluding_header as usize;
        let reencoded_rrd_footer_end = reencoded_rrd_footer_start
            .checked_add(reencoded_stream_footer.rrd_footer_byte_size_excluding_header as usize)
            .unwrap();
        let reencoded_rrd_footer_bytes =
            &msgs_reencoded[reencoded_rrd_footer_start..reencoded_rrd_footer_end];

        {
            let crc = re_log_encoding::StreamFooter::from_rrd_footer_bytes(
                reencoded_rrd_footer_start as u64,
                reencoded_rrd_footer_bytes,
            )
            .crc_excluding_header;
            similar_asserts::assert_eq!(reencoded_stream_footer.crc_excluding_header, crc);
        }

        let reencoded_rrd_footer =
            re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(reencoded_rrd_footer_bytes)
                .unwrap();
        let mut reencoded_rrd_footer = reencoded_rrd_footer.to_application(()).unwrap();

        let reencoded_rrd_manifest_recording = reencoded_rrd_footer
            .manifests
            .remove(&store_id_recording)
            .unwrap();
        let reencoded_rrd_manifest_blueprint = reencoded_rrd_footer
            .manifests
            .remove(&store_id_blueprint)
            .unwrap();

        similar_asserts::assert_eq!(
            batch_to_comparable_string(&rrd_manifest_recording.data),
            batch_to_comparable_string(&reencoded_rrd_manifest_recording.data),
            "Reencoded RRD manifest should be identical to the original one",
        );
        // Same test but check everything, not just the manifest data (we do both cause we want a nice diff for the manifest data)
        similar_asserts::assert_eq!(
            &rrd_manifest_recording,
            &reencoded_rrd_manifest_recording,
            "Reencoded RRD manifest should be identical to the original one",
        );

        similar_asserts::assert_eq!(
            batch_to_comparable_string(&rrd_manifest_blueprint.data),
            batch_to_comparable_string(&reencoded_rrd_manifest_blueprint.data),
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
            is_partial: false,
        },
    }))
    .chain(chunks.map(move |chunk| LogMsg::ArrowMsg(store_id.clone(), chunk)))
}

fn generate_recording_chunks(tuid_prefix: u64) -> impl Iterator<Item = re_log_types::ArrowMsg> {
    use re_log_types::{
        TimeInt, build_frame_nr,
        example_components::{MyColor, MyLabel, MyPoint, MyPoints},
    };

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    let entity_path = "my_entity";

    [
        // Single chunk with sequential, complete data
        {
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

            Chunk::builder_with_id(next_chunk_id(), entity_path)
                .with_sparse_component_batches(
                    next_row_id(),
                    [build_frame_nr(frame1)],
                    [
                        (MyPoints::descriptor_points(), Some(&points1 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors1 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    next_row_id(),
                    [build_frame_nr(frame2)],
                    [
                        (MyPoints::descriptor_points(), Some(&points2 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors2 as _)),
                    ],
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
                    [
                        (MyPoints::descriptor_points(), Some(&points4 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors4 as _)),
                    ],
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
            is_partial: false,
        },
    }))
    .chain(chunks.map(move |chunk| LogMsg::ArrowMsg(store_id.clone(), chunk)))
}

fn generate_blueprint_chunks(tuid_prefix: u64) -> impl Iterator<Item = re_log_types::ArrowMsg> {
    use re_log_types::{EntityPath, TimeInt, build_frame_nr};
    use re_types::blueprint::archetypes::TimePanelBlueprint;

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

fn batch_to_comparable_string(batch: &arrow::array::RecordBatch) -> String {
    re_arrow_util::format_record_batch_opts(
        batch,
        &re_arrow_util::RecordBatchFormatOpts {
            transposed: true,
            width: None,
            max_cell_content_width: 32,
            include_metadata: false,
            include_column_metadata: false,
            trim_field_names: false,
            trim_metadata_keys: false,
            trim_metadata_values: false,
            redact_non_deterministic: false,
        },
    )
    .to_string()
}

fn schema_to_comparable_string(schema: &arrow::datatypes::Schema) -> String {
    let metadata = (!schema.metadata().is_empty()).then(|| {
        format!(
            "top-level metadata: [\n    {}\n]",
            schema
                .metadata()
                .iter()
                .map(|(k, v)| format!("{k}:{v}"))
                .sorted()
                .join("\n    ")
        )
    });

    let mut fields = schema.fields.iter().collect_vec();
    fields.sort_by(|a, b| a.name().cmp(b.name()));
    let fields = fields.into_iter().map(|field| {
        if field.metadata().is_empty() {
            format!(
                "{}: {}",
                field.name(),
                re_arrow_util::format_data_type(field.data_type())
            )
        } else {
            format!(
                "{}: {} [\n    {}\n]",
                field.name(),
                re_arrow_util::format_data_type(field.data_type()),
                field
                    .metadata()
                    .iter()
                    .map(|(k, v)| format!("{k}:{v}"))
                    .sorted()
                    .join("\n    ")
            )
        }
    });

    metadata.into_iter().chain(fields).join("\n")
}
