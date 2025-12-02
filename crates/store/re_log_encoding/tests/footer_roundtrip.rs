use re_chunk::RowId;
use re_log_encoding::{Decodable as _, Encoder, ToApplication as _};
use re_log_types::{LogMsg, StoreId};

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
    let _rrd_footer = rrd_footer.to_application(()).unwrap();
}
