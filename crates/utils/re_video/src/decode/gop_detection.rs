use h264_reader::{
    annexb::AnnexBReader,
    nal::{self, Nal as _},
    push::NalInterest,
};

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
        crate::VideoCodec::H264 => Ok(is_annexb_sample_start_of_gop(sample_data)),
        crate::VideoCodec::H265 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::AV1 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::VP8 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
        crate::VideoCodec::VP9 => Err(StartOfGopDetectionFailure::UnsupportedCodec(codec)),
    }
}

#[derive(Default)]
struct H264GopDetectionState {
    sps_found: bool,
    idr_found: bool,
}

impl h264_reader::push::AccumulatedNalHandler for H264GopDetectionState {
    fn nal(&mut self, nal: nal::RefNal<'_>) -> NalInterest {
        let Ok(nal_header) = nal.header() else {
            return NalInterest::Ignore;
        };
        let nal_unit_type = nal_header.nal_unit_type();

        if nal_unit_type == nal::UnitType::SeqParameterSet {
            self.sps_found = true;
        } else if nal_unit_type == nal::UnitType::SliceLayerWithoutPartitioningIdr {
            self.idr_found = true;
        }

        NalInterest::Ignore
    }
}

impl H264GopDetectionState {
    fn detected_gop(&self) -> bool {
        // We look for one SPS and one IDR frame in this chunk, otherwise we don't count it as a GOP.
        self.sps_found && self.idr_found
    }
}

/// Try to determine whether a frame chunk is the start of a closed GOP.
///
/// Expects Annex B encoded frame.
fn is_annexb_sample_start_of_gop(mut sample_data: &[u8]) -> bool {
    let mut reader = AnnexBReader::accumulate(H264GopDetectionState::default());

    while !sample_data.is_empty() {
        // Don't parse everything at once.
        const MAX_CHUNK_SIZE: usize = 256;
        let chunk_size = MAX_CHUNK_SIZE.min(sample_data.len());

        reader.push(&sample_data[..chunk_size]);

        if reader.nal_handler_ref().detected_gop() {
            return true;
        }

        sample_data = &sample_data[chunk_size..];
    }

    false
}

#[cfg(test)]
mod test {
    use super::is_annexb_sample_start_of_gop;

    #[test]
    fn test_is_annexb_sample_start_of_gop() {
        // Example H.264 Annex B encoded data containing SPS and IDR frame. (ai generated)
        let sample_data = &[
            // SPS NAL unit
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80, // IDR frame NAL unit
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B,
            0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
        ];
        let result = is_annexb_sample_start_of_gop(sample_data);
        assert!(result);

        // Garbage data, still annex b shaped. (ai generated)
        let sample_data = &[
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80,
        ];
        let result = is_annexb_sample_start_of_gop(sample_data);
        assert!(!result);

        // Garbage data, no detectable nalu units.
        let sample_data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A];
        let result = is_annexb_sample_start_of_gop(sample_data);
        assert!(!result);
    }
}
