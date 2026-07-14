use std::io;
use std::io::Cursor;

use scuffle_av1::seq::SequenceHeaderObu;
use scuffle_av1::{ObuHeader, ObuType};
use scuffle_bytes_util::BitReader;

use crate::{ChromaSubsamplingModes, DetectGopStartError, GopStartDetection, VideoEncodingDetails};

/// Try to determine whether an AV1 frame chunk is the start of a GOP.
///
/// This uses a simplified approach that requires both a sequence header OBU and at least one
/// keyframe OBU to be present. If either is missing, we don't consider it a GOP start.
///
/// While it's technically valid for AV1 samples to contain keyframes without a sequence header
/// (relying on decoder state from earlier samples), we require it here because we need to extract
/// frame dimensions, bit depth, and chroma subsampling info. Without the sequence header, we can't
/// reliably determine these encoding details.
///
/// We intentionally ignore `INTRA_ONLY` frames even though they're also independently decodable.
/// This is because `INTRA_ONLY` frames can technically reference existing decoder state (like
/// previously decoded reference frames or sequence headers), so they're not truly independent
/// GOP boundaries. Only `KEY_FRAME` OBUs represent a clean break where decoding can start fresh
/// without any prior state.
pub fn detect_av1_keyframe_start(data: &[u8]) -> Result<GopStartDetection, DetectGopStartError> {
    let mut keyframe_found = false;
    let mut video_encoding_details: Option<VideoEncodingDetails> = None;

    let mut cursor = Cursor::new(data);

    loop {
        let Ok(header) = ObuHeader::parse(&mut cursor) else {
            // If we already know it's a GOP start, ignore trailing garbage.
            if keyframe_found && video_encoding_details.is_some() {
                break;
            }

            // Otherwise, treat bad / truncated data as not a GOP start.
            return Ok(GopStartDetection::NotStartOfGop);
        };

        let payload_start = cursor.position();
        let obu_size = header.size.unwrap_or_default();

        match header.obu_type {
            ObuType::SequenceHeader => {
                let seq = SequenceHeaderObu::parse(header, &mut cursor)
                    .map_err(DetectGopStartError::Av1ParserError)?;

                let profile = seq.seq_profile;
                let bit_depth = seq.color_config.bit_depth as u8;

                // Use the first operating point's level and tier for the codec string.
                // Format: av01.P.LLT.DD
                // See https://aomediacodec.github.io/av1-isobmff/#codecsparam
                let (level, tier) = seq
                    .operating_points
                    .first()
                    .map(|op| (op.seq_level_idx, if op.seq_tier { "H" } else { "M" }))
                    .unwrap_or((1, "M"));

                video_encoding_details = Some(VideoEncodingDetails {
                    codec_string: format!("av01.{profile}.{level:02}{tier}.{bit_depth:02}"),
                    coded_dimensions: [seq.max_frame_width as u16, seq.max_frame_height as u16],
                    bit_depth: Some(bit_depth),
                    chroma_subsampling: Some(chroma_mode_from_color_config(&seq.color_config)),
                    stsd: None,
                });
            }
            ObuType::Frame | ObuType::FrameHeader
                if is_keyframe(&mut cursor).map_err(DetectGopStartError::Av1ParserError)? =>
            {
                keyframe_found = true;
            }
            _ => {
                // Skip other OBUs
            }
        }

        if header.size.is_some() {
            // OBU has a known size: jump to the end of its payload.
            cursor.set_position(payload_start + obu_size);
        } else {
            re_log::warn_once!(
                "AV1 sample contains an OBU without a size field. Keyframe \
                 detection may be incomplete."
            );
            break;
        }
    }

    if keyframe_found && let Some(details) = video_encoding_details {
        Ok(GopStartDetection::StartOfGop(details))
    } else {
        Ok(GopStartDetection::NotStartOfGop)
    }
}

/// Determine if the frame is a keyframe based on the OBU type and its content.
#[inline]
fn is_keyframe<R: io::Read>(reader: &mut R) -> io::Result<bool> {
    let mut reader = BitReader::new(reader);

    // only present for frame header OBUs, skip for frame OBUs
    let show_existing_frame = reader.read_bit()?;

    if show_existing_frame {
        return Ok(false);
    }

    // frame_type (2 bits)
    // 0 = KEY_FRAME
    // 1 = INTER_FRAME
    // 2 = INTRA_ONLY_FRAME
    // 3 = SWITCH_FRAME
    let frame_type = reader.read_bits(2)?;
    Ok(frame_type == 0)
}

#[inline]
fn chroma_mode_from_color_config(config: &scuffle_av1::seq::ColorConfig) -> ChromaSubsamplingModes {
    let subsampling_x = config.subsampling_x;
    let subsampling_y = config.subsampling_y;

    if config.mono_chrome {
        ChromaSubsamplingModes::Monochrome
    } else if !subsampling_x && !subsampling_y {
        // Not subsampling => 4:4:4
        ChromaSubsamplingModes::Yuv444
    } else if subsampling_x && !subsampling_y {
        // Subsampling in X only => 4:2:2
        ChromaSubsamplingModes::Yuv422
    } else {
        // Subsampling in both X and Y => 4:2:0
        ChromaSubsamplingModes::Yuv420
    }
}

/// Small 64x64 AV1 keyframe for testing (generated with ffmpeg)
///
/// This contains a Sequence Header OBU followed by a keyframe
pub const AV1_TEST_KEYFRAME: &[u8] = &[
    0x12, 0x00, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x02, 0xAF, 0xFF, 0x9F, 0xFF, 0x30, 0x08, 0x32, 0x14,
    0x10, 0x00, 0xC0, 0x00, 0x00, 0x02, 0x80, 0x00, 0x00, 0x0A, 0x05, 0x76, 0xA4, 0xD6, 0x2F, 0x1F,
    0xFA, 0x1E, 0x3C, 0xD8,
];

/// AV1 inter-frame (non-keyframe) for testing (generated with ffmpeg)
///
/// This contains temporal Delimiter OBU + Frame OBU with inter-frame
pub const AV1_TEST_INTER_FRAME: &[u8] = &[
    0x12, 0x00, 0x32, 0x12, 0x30, 0x03, 0xC0, 0x80, 0x00, 0x00, 0x06, 0xC0, 0x00, 0x00, 0x02, 0x80,
    0x00, 0x00, 0x80, 0x00, 0x99, 0x10,
];

#[cfg(test)]
mod test {
    use super::{GopStartDetection, detect_av1_keyframe_start};

    #[test]
    fn test_detect_av1_keyframe_start() {
        let result = detect_av1_keyframe_start(super::AV1_TEST_KEYFRAME);

        match result {
            Ok(GopStartDetection::StartOfGop(details)) => {
                // Verify we got expected details from the AV1 stream
                assert_eq!(details.codec_string, "av01.0.00M.08");
                assert_eq!(details.coded_dimensions, [64, 64]);

                assert_eq!(details.bit_depth, Some(8));
            }
            Err(err) => panic!("Failed to parse valid AV1 data: {err}"),
            Ok(GopStartDetection::NotStartOfGop) => {
                panic!("Expected to detect GOP start but got `NotStartOfGop`")
            }
        }
    }

    #[test]
    fn test_detect_av1_empty_data() {
        let result = detect_av1_keyframe_start(&[]);
        assert!(matches!(result, Ok(GopStartDetection::NotStartOfGop)));
    }

    #[test]
    fn test_detect_av1_invalid_data() {
        // Random invalid data, make sure we handle that gracefully and don't panic
        let invalid_data = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let result = detect_av1_keyframe_start(invalid_data);

        assert!(matches!(result, Ok(GopStartDetection::NotStartOfGop)));
    }

    /// AV1 sample with three sequential Frame OBUs and no Sequence Header.
    ///
    /// Regression fixture for the OBU walker cursor-drift bug.
    const AV1_MULTI_FRAME_DRIFT_REPRO: &[u8] = &[
        0x32, 0x30, 0x28, 0x9C, 0xC2, 0x05, 0x69, 0x7B, 0x24, 0x6A, 0x04, 0x1C, 0x71, 0xC7, 0x03,
        0x00, 0x01, 0x00, 0x20, 0x04, 0x80, 0x60, 0xC0, 0x00, 0x00, 0x62, 0x73, 0x15, 0x73, 0x05,
        0x58, 0x72, 0x84, 0xD9, 0xD5, 0xF4, 0x6A, 0xBB, 0xF3, 0xB5, 0x5E, 0xF0, 0xF6, 0xC2, 0x6B,
        0x38, 0x6B, 0x70, 0xD9, 0x4C, 0x32, 0x1B, 0x28, 0x94, 0x60, 0x05, 0x69, 0x7B, 0x24, 0x72,
        0x04, 0x1E, 0x79, 0xE7, 0x83, 0x00, 0x01, 0x00, 0x20, 0x04, 0x80, 0x60, 0xC0, 0x00, 0x94,
        0xB2, 0x5B, 0xEA, 0xFA, 0x32, 0x4B, 0x31, 0x26, 0x80, 0x0A, 0xD2, 0xF6, 0x48, 0xE4, 0x08,
        0x3C, 0xF3, 0xCF, 0x06, 0x00, 0x02, 0x00, 0x40, 0x09, 0x00, 0xC1, 0x80, 0x00, 0x7B, 0x4D,
        0x69, 0xD2, 0xD7, 0xC7, 0x6D, 0x2F, 0xB4, 0xF2, 0xDA, 0xD1, 0xDC, 0x5A, 0xD9, 0x45, 0x0F,
        0xA2, 0xB7, 0x98, 0xD0, 0x19, 0xF6, 0x3C, 0x42, 0xBB, 0x5F, 0x5E, 0xE1, 0xF3, 0xEB, 0xF2,
        0xCE, 0x63, 0x4D, 0xD7, 0xC5, 0xEA, 0x0A, 0xDE, 0xFA, 0x76, 0x15, 0xAC, 0xB8, 0x85, 0x88,
        0x9C, 0x7D, 0x59, 0x63, 0x45, 0xA8,
    ];

    #[test]
    fn test_detect_av1_multi_frame_does_not_drift() {
        let result = detect_av1_keyframe_start(AV1_MULTI_FRAME_DRIFT_REPRO);
        assert!(
            matches!(result, Ok(GopStartDetection::NotStartOfGop)),
            "expected NotStartOfGop, got {result:?}"
        );
    }

    #[test]
    fn test_detect_av1_non_keyframe() {
        let result = detect_av1_keyframe_start(super::AV1_TEST_INTER_FRAME);

        // Should return a `NotStartOfGop` or error
        match result {
            Ok(GopStartDetection::StartOfGop(_)) => {
                panic!("Should not detect GOP start without keyframe");
            }
            Err(_) | Ok(GopStartDetection::NotStartOfGop) => {
                // Expected outcome
            }
        }
    }
}
