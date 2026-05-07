use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};

use re_log_types::{LogMsg, StoreId};

use crate::rrd::{
    CodecError, Decodable as _, DecoderEntrypoint as _, MessageHeader, MessageKind, StreamFooter,
    StreamHeader,
};
use crate::{CachingApplicationIdInjector, RrdFooter, ToApplication as _};

/// Read the full RRD footer from an open file using seek-based I/O.
///
/// The file position is moved during reading (seeks to header, footer, payload).
///
/// Returns `Ok(None)` if the file is a valid RRD but has no footer (legacy RRD).
/// Returns `Err` if the file is not a valid RRD or is corrupted.
///
/// The returned [`RrdFooter`] contains manifests for ALL stores in the file.
/// Caller is responsible for selecting the desired store.
pub fn read_rrd_footer(file: &mut File) -> Result<Option<RrdFooter>, CodecError> {
    let file_len = file.metadata()?.len();

    // 1. Validate the StreamHeader to confirm this is actually an RRD file.
    if file_len < StreamHeader::ENCODED_SIZE_BYTES as u64 {
        return Err(CodecError::FrameDecoding(
            "file too small to be an RRD".to_owned(),
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut header_buf = [0u8; StreamHeader::ENCODED_SIZE_BYTES];
    file.read_exact(&mut header_buf)?;
    StreamHeader::from_rrd_bytes(&header_buf)?; // validates FourCC + version

    // 2. Read the StreamFooter from the end of the file.
    if file_len < StreamFooter::ENCODED_SIZE_BYTES as u64 {
        return Ok(None); // File too small to have a footer.
    }
    // SAFETY: ENCODED_SIZE_BYTES is a small constant (32), fits in i64.
    #[expect(clippy::cast_possible_wrap)]
    file.seek(SeekFrom::End(-(StreamFooter::ENCODED_SIZE_BYTES as i64)))?;
    let mut footer_buf = [0u8; StreamFooter::ENCODED_SIZE_BYTES];
    file.read_exact(&mut footer_buf)?;

    let Ok(stream_footer) = StreamFooter::from_rrd_bytes(&footer_buf) else {
        return Ok(None); // Valid RRD, but no footer (legacy).
    };

    // 2. For each entry, read and validate the RrdFooter payload.
    //    In practice there is always exactly one entry.
    let Some(entry) = stream_footer.entries.first() else {
        return Ok(None);
    };

    let span = &entry.rrd_footer_byte_span_from_start_excluding_header;
    let payload_len = usize::try_from(span.len)?;

    // Sanity check: payload must fit within the file.
    if span.start + span.len > file_len {
        return Err(CodecError::FrameDecoding(format!(
            "RrdFooter payload span ({start}..{end}) exceeds file size ({file_len})",
            start = span.start,
            end = span.start + span.len,
        )));
    }

    // 3. Seek to the RrdFooter payload and read it.
    file.seek(SeekFrom::Start(span.start))?;
    let mut payload_buf = vec![0u8; payload_len];
    file.read_exact(&mut payload_buf)?;

    // 4. Validate CRC.
    let actual_crc = StreamFooter::compute_crc(&payload_buf);
    if actual_crc != entry.crc_excluding_header {
        return Err(CodecError::CrcMismatch {
            expected: entry.crc_excluding_header,
            got: actual_crc,
        });
    }

    // 5. Decode protobuf RrdFooter → application-level RrdFooter.
    let transport_footer = re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(&payload_buf)?;
    let rrd_footer = transport_footer.to_application(())?;

    Ok(Some(rrd_footer))
}

/// Enumerate all [`StoreId`]s present in an RRD file, without reading chunk data.
///
/// - **With footer** (modern RRDs): reads the footer and returns the keys of its manifests map.
///   Cheap: 3 seeks, no chunk data read. All stores are visible regardless of message ordering.
/// - **Without footer** (legacy RRDs): walks message frames, decoding only `SetStoreInfo`
///   payloads and seeking past everything else. All `SetStoreInfo`s are discovered regardless
///   of how they interleave with `ArrowMsg`s.
///
/// The file position is moved during reading. The returned list is sorted by [`StoreId`]'s
/// natural order for determinism.
pub fn enumerate_rrd_stores(file: &mut File) -> Result<Vec<StoreId>, CodecError> {
    // Try footer first (cheap: 3 seeks, no chunk data read).
    if let Some(footer) = read_rrd_footer(file)? {
        let mut store_ids: Vec<StoreId> = footer.manifests.into_keys().collect();
        store_ids.sort();
        return Ok(store_ids);
    }

    enumerate_legacy_stores(file)
}

/// Legacy (no-footer) enumeration: walk message frames without decoding chunk data.
///
/// We read each 16-byte [`MessageHeader`], decode only `SetStoreInfo` payloads, and
/// `seek()` past `ArrowMsg` / `BlueprintActivationCommand` payloads. Cost is
/// `O(num_messages * 16 bytes)` of frame reads + small `SetStoreInfo` payload decodes.
/// No Arrow IPC decoding ever happens.
///
/// The same `StoreId` can appear in multiple `SetStoreInfo` messages (e.g. after a
/// flush/reconnect); the returned list is deduplicated.
fn enumerate_legacy_stores(file: &mut File) -> Result<Vec<StoreId>, CodecError> {
    // `read_rrd_footer` already moved the file position — seek back to start.
    file.seek(SeekFrom::Start(0))?;

    // Read and validate the StreamHeader; it also carries the crate version we need when
    // decoding individual message payloads (for version-dependent migrations).
    let mut stream_header_buf = [0u8; StreamHeader::ENCODED_SIZE_BYTES];
    file.read_exact(&mut stream_header_buf)?;
    let stream_header = StreamHeader::from_rrd_bytes(&stream_header_buf)?;
    let (version, _options) = stream_header.to_version_and_options()?;

    let mut store_ids = Vec::new();
    let mut app_id_cache = CachingApplicationIdInjector::default();

    loop {
        let mut msg_header_buf = [0u8; MessageHeader::ENCODED_SIZE_BYTES];
        match file.read_exact(&mut msg_header_buf) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(CodecError::Io(err)),
        }
        let header = MessageHeader::from_rrd_bytes(&msg_header_buf)?;

        match header.kind {
            MessageKind::End => break,

            MessageKind::SetStoreInfo => {
                let payload_len = usize::try_from(header.len).map_err(CodecError::Overflow)?;
                let mut payload = vec![0u8; payload_len];
                file.read_exact(&mut payload)?;
                let byte_span = re_chunk::Span {
                    start: 0,
                    len: header.len,
                };
                if let Some(LogMsg::SetStoreInfo(set_store_info)) = LogMsg::decode(
                    bytes::Bytes::from(payload),
                    byte_span,
                    MessageKind::SetStoreInfo,
                    &mut app_id_cache,
                    Some(version),
                )? {
                    store_ids.push(set_store_info.info.store_id);
                }
            }

            MessageKind::ArrowMsg | MessageKind::BlueprintActivationCommand => {
                let offset = i64::try_from(header.len).map_err(CodecError::Overflow)?;
                file.seek(SeekFrom::Current(offset))?;
            }
        }
    }

    store_ids.sort();
    store_ids.dedup();
    Ok(store_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rrd::test_util::{encode_test_rrd, encode_test_rrd_to_file, make_test_chunks};

    #[test]
    fn test_read_footer_roundtrip() {
        let chunks = make_test_chunks(5);
        let (file, _store_id) = encode_test_rrd(&chunks);

        let footer = read_rrd_footer(&mut File::open(file.path()).unwrap()).unwrap();
        assert!(footer.is_some(), "Footer should be present");
        let footer = footer.unwrap();
        assert!(
            !footer.manifests.is_empty(),
            "Should have at least one manifest"
        );
    }

    #[test]
    fn test_read_footer_no_footer() {
        // Needs with_footer: false, so uses the lower-level helper.
        let file = tempfile::NamedTempFile::new().unwrap();
        let chunks = make_test_chunks(3);
        encode_test_rrd_to_file(file.path(), &chunks, false);

        let footer = read_rrd_footer(&mut File::open(file.path()).unwrap()).unwrap();
        assert!(footer.is_none(), "Legacy RRD should have no footer");
    }

    #[test]
    fn test_enumerate_rrd_stores_footer_path() {
        let chunks = make_test_chunks(5);
        let (file, store_id) = encode_test_rrd(&chunks);

        let ids = enumerate_rrd_stores(&mut File::open(file.path()).unwrap()).unwrap();
        assert_eq!(ids, vec![store_id]);
    }

    #[test]
    fn test_enumerate_rrd_stores_legacy_path() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let chunks = make_test_chunks(3);
        encode_test_rrd_to_file(file.path(), &chunks, false);

        let ids = enumerate_rrd_stores(&mut File::open(file.path()).unwrap()).unwrap();
        assert_eq!(
            ids.len(),
            1,
            "Legacy RRD should have its single store discovered"
        );
    }

    /// Cross-check: the legacy (frame-scan) and modern (footer) paths must return identical
    /// results on the same logical RRD content.
    #[test]
    fn test_enumerate_rrd_stores_footer_vs_legacy_agree() {
        let chunks = make_test_chunks(5);
        let store_id =
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "cross_check");

        let with_footer = tempfile::NamedTempFile::new().unwrap();
        let without_footer = tempfile::NamedTempFile::new().unwrap();
        crate::rrd::test_util::encode_test_rrd_to_file_with_options(
            with_footer.path(),
            &chunks,
            &store_id,
            true,
            crate::EncodingOptions::PROTOBUF_COMPRESSED,
        );
        crate::rrd::test_util::encode_test_rrd_to_file_with_options(
            without_footer.path(),
            &chunks,
            &store_id,
            false,
            crate::EncodingOptions::PROTOBUF_COMPRESSED,
        );

        let ids_footer =
            enumerate_rrd_stores(&mut File::open(with_footer.path()).unwrap()).unwrap();
        let ids_legacy =
            enumerate_rrd_stores(&mut File::open(without_footer.path()).unwrap()).unwrap();

        assert_eq!(ids_footer, vec![store_id]);
        assert_eq!(
            ids_footer, ids_legacy,
            "footer path and legacy path must agree on the same content"
        );
    }

    /// Legacy RRD where the *same* `StoreId` is announced by several `SetStoreInfo`
    /// messages (can happen e.g. after a flush/reconnect). Enumeration must dedup.
    #[test]
    fn test_enumerate_rrd_stores_legacy_duplicate_set_store_info() {
        use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};

        let chunks = make_test_chunks(2);
        let store_id = StoreId::random(StoreKind::Recording, "dup_test");

        let file = tempfile::NamedTempFile::new().unwrap();
        {
            let mut out = std::fs::File::create(file.path()).unwrap();
            let mut encoder = crate::Encoder::new_eager(
                re_build_info::CrateVersion::LOCAL,
                crate::EncodingOptions::PROTOBUF_COMPRESSED,
                &mut out,
            )
            .unwrap();
            encoder.do_not_emit_footer();

            let info = StoreInfo::new(store_id.clone(), StoreSource::Unknown);
            // Three SetStoreInfos for the same store, interleaved with chunks.
            encoder
                .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                    row_id: *re_chunk::RowId::ZERO,
                    info: info.clone(),
                }))
                .unwrap();
            let arrow_msg_0 = chunks[0].to_arrow_msg().unwrap();
            encoder
                .append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg_0))
                .unwrap();
            encoder
                .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                    row_id: *re_chunk::RowId::ZERO,
                    info: info.clone(),
                }))
                .unwrap();
            let arrow_msg_1 = chunks[1].to_arrow_msg().unwrap();
            encoder
                .append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg_1))
                .unwrap();
            encoder
                .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                    row_id: *re_chunk::RowId::ZERO,
                    info,
                }))
                .unwrap();
            encoder.finish().unwrap();
        }

        let ids = enumerate_rrd_stores(&mut File::open(file.path()).unwrap()).unwrap();
        assert_eq!(
            ids,
            vec![store_id],
            "duplicate SetStoreInfos for the same StoreId must be deduped"
        );
    }

    /// Two-store RRD with `SetStoreInfo` messages interleaved with `ArrowMsg`s. Both the
    /// legacy (no-footer) path and the modern (footer) path must discover both stores and
    /// return the same result.
    #[test]
    fn test_enumerate_rrd_stores_interleaved_footer_vs_legacy() {
        use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};

        let chunks_a = make_test_chunks(2);
        let chunks_b = make_test_chunks(2);
        let store_a = StoreId::random(StoreKind::Recording, "test_a");
        let store_b = StoreId::random(StoreKind::Recording, "test_b");

        // Writes interleaved content: SetStoreInfo(A), chunks(A), SetStoreInfo(B), chunks(B).
        let write_interleaved = |path: &std::path::Path, with_footer: bool| {
            let mut out = std::fs::File::create(path).unwrap();
            let mut encoder = crate::Encoder::new_eager(
                re_build_info::CrateVersion::LOCAL,
                crate::EncodingOptions::PROTOBUF_COMPRESSED,
                &mut out,
            )
            .unwrap();
            if !with_footer {
                encoder.do_not_emit_footer();
            }

            let info_a = StoreInfo::new(store_a.clone(), StoreSource::Unknown);
            encoder
                .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                    row_id: *re_chunk::RowId::ZERO,
                    info: info_a,
                }))
                .unwrap();
            for chunk in &chunks_a {
                let arrow_msg = chunk.to_arrow_msg().unwrap();
                encoder
                    .append(&LogMsg::ArrowMsg(store_a.clone(), arrow_msg))
                    .unwrap();
            }
            let info_b = StoreInfo::new(store_b.clone(), StoreSource::Unknown);
            encoder
                .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                    row_id: *re_chunk::RowId::ZERO,
                    info: info_b,
                }))
                .unwrap();
            for chunk in &chunks_b {
                let arrow_msg = chunk.to_arrow_msg().unwrap();
                encoder
                    .append(&LogMsg::ArrowMsg(store_b.clone(), arrow_msg))
                    .unwrap();
            }
            encoder.finish().unwrap();
        };

        let with_footer = tempfile::NamedTempFile::new().unwrap();
        let without_footer = tempfile::NamedTempFile::new().unwrap();
        write_interleaved(with_footer.path(), true);
        write_interleaved(without_footer.path(), false);

        let ids_footer =
            enumerate_rrd_stores(&mut File::open(with_footer.path()).unwrap()).unwrap();
        let ids_legacy =
            enumerate_rrd_stores(&mut File::open(without_footer.path()).unwrap()).unwrap();

        let mut expected = vec![store_a, store_b];
        expected.sort();
        assert_eq!(ids_footer, expected, "footer path must find both stores");
        assert_eq!(
            ids_legacy, expected,
            "legacy path must find both stores despite interleaving"
        );
        assert_eq!(
            ids_footer, ids_legacy,
            "footer and legacy paths must agree on the same content"
        );
    }

    #[test]
    fn test_read_footer_not_an_rrd() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"this is not an rrd file at all").unwrap();

        let result = read_rrd_footer(&mut File::open(file.path()).unwrap());
        assert!(result.is_err(), "Non-RRD file should return an error");
    }

    #[test]
    fn test_read_footer_too_small() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"tiny").unwrap();

        let result = read_rrd_footer(&mut File::open(file.path()).unwrap());
        assert!(
            result.is_err(),
            "File too small for StreamHeader should error"
        );
    }

    #[test]
    fn test_read_footer_corrupted_crc() {
        let chunks = make_test_chunks(3);
        let (file, _store_id) = encode_test_rrd(&chunks);

        let mut data = std::fs::read(file.path()).unwrap();
        let file_len = data.len();

        let footer_bytes = &data[file_len - StreamFooter::ENCODED_SIZE_BYTES..];
        let stream_footer = StreamFooter::from_rrd_bytes(footer_bytes).unwrap();
        let entry = &stream_footer.entries[0];
        let payload_start = entry.rrd_footer_byte_span_from_start_excluding_header.start as usize;

        // Flip a byte in the payload.
        data[payload_start] ^= 0xFF;
        std::fs::write(file.path(), &data).unwrap();

        let result = read_rrd_footer(&mut File::open(file.path()).unwrap());
        assert!(
            matches!(result, Err(CodecError::CrcMismatch { .. })),
            "Expected CRC mismatch, got: {result:?}"
        );
    }
}
