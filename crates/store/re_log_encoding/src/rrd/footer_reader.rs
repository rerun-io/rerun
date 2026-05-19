use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};

use crate::rrd::{CodecError, Decodable as _, StreamFooter, StreamHeader};
use crate::{RrdFooter, ToApplication as _};

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
