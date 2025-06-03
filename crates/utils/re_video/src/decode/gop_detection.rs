use crate::decode::nalu::{NAL_START_CODE, NAL_START_CODE_SHORT};

use super::nalu::{NalHeader, NalUnitType};

/// Failure reason for [`is_sample_start_of_gop`].
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum StartOfGopDetectionFailure {
    #[error("Detection not supported for codec: {0:?}")]
    UnsupportedCodec(crate::VideoCodec),

    #[error("Expected sample to be at least one NAL unit.")]
    ExpectedAnnexBNalStartCode,

    #[error("Expected NAL unit to be at least one byte.")]
    ZeroSizeNalUnit,
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
        crate::VideoCodec::Av1 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::H265 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::Vp8 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::Vp9 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
    }
}

/// Try to determine whether a frame chunk is the start of a closed GOP.
///
/// Expects Annex B encoded frame.
fn is_annexb_sample_start_of_gop(sample_data: &[u8]) -> Result<bool, StartOfGopDetectionFailure> {
    let nal_units = divide_into_nal_units(sample_data)?;

    // We look for one SPS and one IDR frame in this chunk, otherwise we don't count it as a GOP.
    let mut sps_found = false;
    let mut idr_found = false;
    for nal_unit in nal_units {
        let first_byte = nal_unit
            .first()
            .ok_or(StartOfGopDetectionFailure::ZeroSizeNalUnit)?;
        let header = NalHeader(*first_byte);

        if header.unit_type() == NalUnitType::SequenceParameterSet {
            sps_found = true;
        } else if header.unit_type() == NalUnitType::CodedSliceOfAnIDRPicture {
            idr_found = true;
        }

        if sps_found && idr_found {
            return Ok(true);
        }
    }

    Ok(false)
}

fn divide_into_nal_units(
    sample_data: &[u8],
) -> Result<smallvec::SmallVec<[&[u8]; 2]>, StartOfGopDetectionFailure> {
    // See https://membrane.stream/learn/h264/3 for an explation of Annex B.
    let mut nal_units = smallvec::SmallVec::new();
    if sample_data.len() < NAL_START_CODE.len() {
        // Need at least enough for one short start code and one one-byte header.
        return Err(StartOfGopDetectionFailure::ExpectedAnnexBNalStartCode);
    }

    let mut nal_unit_start_pos = if &sample_data[0..NAL_START_CODE.len()] == NAL_START_CODE {
        NAL_START_CODE.len()
    } else if &sample_data[0..NAL_START_CODE_SHORT.len()] == NAL_START_CODE_SHORT {
        NAL_START_CODE_SHORT.len()
    } else {
        return Err(StartOfGopDetectionFailure::ExpectedAnnexBNalStartCode);
    };

    let mut pos = nal_unit_start_pos;
    while pos < sample_data.len() - NAL_START_CODE.len() {
        if &sample_data[pos..pos + NAL_START_CODE.len()] == NAL_START_CODE {
            nal_units.push(&sample_data[nal_unit_start_pos..pos]);
            pos += NAL_START_CODE.len();
            nal_unit_start_pos = pos;
        } else if &sample_data[pos..pos + NAL_START_CODE_SHORT.len()] == NAL_START_CODE_SHORT {
            nal_units.push(&sample_data[nal_unit_start_pos..pos]);
            pos += NAL_START_CODE_SHORT.len();
            nal_unit_start_pos = pos;
        } else {
            pos += 1;
        }
    }
    nal_units.push(&sample_data[nal_unit_start_pos..]);

    Ok(nal_units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_divide_into_nal_units() {
        let sample_data = b"\x00\x00\x00\x01\x01\x02\x00\x00\x01\x03\x00\x00\x00\x01\x04\x06\x06";
        let nal_units = divide_into_nal_units(sample_data).unwrap();
        assert_eq!(nal_units.len(), 3);
        assert_eq!(nal_units[0], b"\x01\x02");
        assert_eq!(nal_units[1], b"\x03");
        assert_eq!(nal_units[2], b"\x04\x06\x06");

        let broken_sample_data = b"\x00\x01\x00\x01";
        assert_eq!(
            divide_into_nal_units(broken_sample_data),
            Err(StartOfGopDetectionFailure::ExpectedAnnexBNalStartCode)
        );
    }
}
