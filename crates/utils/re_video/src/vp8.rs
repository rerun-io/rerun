use crate::{GopStartDetection, VideoEncodingDetails};

pub fn detect_vp8_gop(data: &[u8]) -> GopStartDetection {
    // VP8 keyframes start with a 3-byte frame tag, followed by the keyframe
    // start code and coded dimensions.
    // See https://datatracker.ietf.org/doc/html/rfc6386#section-9.1
    if data.len() < 10 {
        return GopStartDetection::NotStartOfGop;
    }

    let is_interframe = data[0] & 0x01 != 0;
    if is_interframe {
        return GopStartDetection::NotStartOfGop;
    }

    if data[3..6] != [0x9d, 0x01, 0x2a] {
        return GopStartDetection::NotStartOfGop;
    }

    let width = u16::from_le_bytes([data[6], data[7]]) & 0x3fff;
    let height = u16::from_le_bytes([data[8], data[9]]) & 0x3fff;
    if width == 0 || height == 0 {
        return GopStartDetection::NotStartOfGop;
    }

    // Per https://www.w3.org/TR/webcodecs-vp8-codec-registration/#fully-qualified-codec-strings
    // and https://www.ffmpeg.org/doxygen/trunk/vpcc_8c_source.html :
    // VP8's WebCodecs codec string is the bare `"vp8"`, with no `.profile.level.depth` suffix.
    let codec_string = "vp8".to_owned();
    GopStartDetection::StartOfGop(VideoEncodingDetails {
        codec_string,
        coded_dimensions: [width, height],
        // VP8 is always 8-bit YUV 4:2:0. See RFC 6386 §2.
        bit_depth: Some(8),
        chroma_subsampling: Some(crate::ChromaSubsamplingModes::Yuv420),
        stsd: None,
    })
}

/// Returns `true` if the raw VP8 frame data begins with a keyframe.
///
/// See <https://datatracker.ietf.org/doc/html/rfc6386#section-9.1>.
pub fn vp8_is_keyframe(data: &[u8]) -> bool {
    if data.len() < 3 {
        return false;
    }
    (data[0] & 0x01) == 0
}
