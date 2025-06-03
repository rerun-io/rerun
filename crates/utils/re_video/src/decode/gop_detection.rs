use crate::decode::nalu::iter_annex_b_nal_units;

use h264_reader::nal::{NalHeader, UnitType};

/// Failure reason for [`is_sample_start_of_gop`].
#[derive(thiserror::Error, Debug)]
pub enum StartOfGopDetectionFailure {
    #[error("Detection not supported for codec: {0:?}")]
    UnsupportedCodec(crate::VideoCodec),

    #[error("NAL header error: {0:?}")]
    NalHeaderError(h264_reader::nal::NalHeaderError),
}

/// Try to determine whether a frame chunk is the start of a GOP.
///
/// This is a best effort attempt to determine this, but we won't always be able to.
#[inline]
pub fn is_sample_start_of_gop(
    sample_data: &[u8],
    codec: crate::VideoCodec,
) -> Result<bool, StartOfGopDetectionFailure> {
    #[expect(clippy::match_same_arms)]
    match codec {
        crate::VideoCodec::H264 => is_annexb_sample_start_of_gop(sample_data),
        crate::VideoCodec::H265 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::AV1 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::VP8 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::VP9 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
    }
}

/// Try to determine whether a frame chunk is the start of a closed GOP.
///
/// Expects Annex B encoded frame.
fn is_annexb_sample_start_of_gop(sample_data: &[u8]) -> Result<bool, StartOfGopDetectionFailure> {
    // We look for one SPS and one IDR frame in this chunk, otherwise we don't count it as a GOP.
    let mut sps_found = false;
    let mut idr_found = false;
    for nal_unit in iter_annex_b_nal_units(sample_data) {
        debug_assert!(
            !nal_unit.is_empty(),
            "NAL unit is empty despite `iter_annex_b_nal_units`'s guarantee not to return empty units"
        );

        let header =
            NalHeader::new(nal_unit[0]).map_err(StartOfGopDetectionFailure::NalHeaderError)?;
        if header.nal_unit_type() == UnitType::SeqParameterSet {
            sps_found = true;
        } else if header.nal_unit_type() == UnitType::SliceLayerWithoutPartitioningIdr {
            idr_found = true;
        }

        if sps_found && idr_found {
            return Ok(true);
        }
    }

    Ok(false)
}
