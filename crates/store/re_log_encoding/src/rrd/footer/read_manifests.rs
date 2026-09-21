use itertools::Itertools as _;
use re_chunk_index::RawRrdManifest;

use crate::{
    CodecError, CodecResult, Decodable as _, StreamFooter, StreamFooterEntry, ToApplication as _,
};

/// High-level helper to parse [`RawRrdManifest`]s from raw RRD bytes.
///
/// This does not decode all the data, but rather goes straight to the RRD footer (if any).
///
/// * Returns an empty `Vec` if no valid footer was found.
/// * Returns an error if either the footer or any of the manifests are corrupt.
///
/// Usage:
/// ```text,ignore
/// let rrd_bytes = std::fs::read("/path/to/my/recording.rrd");
/// let rrd_manifests = read_raw_rrd_manifests(&rrd_bytes)?;
/// let rrd_manifest = rrd_manifests
///     .into_iter()
///     .find(|m| m.store_id.kind() == StoreKind::Recording)?;
/// ```
pub fn read_raw_rrd_manifests(rrd_bytes: &[u8]) -> CodecResult<Vec<RawRrdManifest>> {
    let stream_footer = match StreamFooter::from_rrd_bytes(rrd_bytes) {
        Ok(footer) => footer,

        // That was in fact _not_ a footer.
        Err(CodecError::FrameDecoding(_)) => return Ok(vec![]),

        Err(err) => Err(err)?,
    };

    let mut manifests = Vec::new();

    for entry in stream_footer.entries {
        let StreamFooterEntry {
            rrd_footer_byte_span_from_start_excluding_header,
            crc_excluding_header,
        } = entry;

        let rrd_footer_byte_span = rrd_footer_byte_span_from_start_excluding_header;

        let rrd_footer_byte_span = rrd_footer_byte_span.try_cast::<usize>().ok_or_else(|| {
            CodecError::FrameDecoding("RRD footer too large for native bit width".to_owned())
        })?;

        let rrd_footer_bytes = rrd_footer_byte_span
            .try_slice(rrd_bytes)
            .ok_or_else(|| {
                CodecError::FrameDecoding(format!(
                    "RRD footer span {rrd_footer_byte_span:?} is out of bounds for a stream of {} bytes",
                    rrd_bytes.len()
                ))
            })?;

        let crc = StreamFooter::compute_crc(rrd_footer_bytes);
        if crc != crc_excluding_header {
            return Err(CodecError::CrcMismatch {
                expected: crc_excluding_header,
                got: crc,
            });
        }

        let rrd_footer = re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(rrd_footer_bytes)?;
        let new_manifests: Vec<_> = rrd_footer
            .manifests
            .iter()
            .map(|manifest| manifest.to_application(()))
            .try_collect()?;
        manifests.extend(new_manifests);
    }

    Ok(manifests)
}
