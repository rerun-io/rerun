use std::io;
use std::io::{Cursor, SeekFrom};

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

        let obu_size = header.size.unwrap_or_default();

        match header.obu_type {
            ObuType::SequenceHeader => {
                let seq = SequenceHeaderObu::parse(header, &mut cursor)
                    .map_err(DetectGopStartError::Av1ParserError)?;

                video_encoding_details = Some(VideoEncodingDetails {
                    codec_string: "av01".to_owned(),
                    coded_dimensions: [seq.max_frame_width as u16, seq.max_frame_height as u16],
                    bit_depth: Some(seq.color_config.bit_depth as u8),
                    chroma_subsampling: Some(chroma_mode_from_color_config(&seq.color_config)),
                    stsd: None,
                });

                continue;
            }
            ObuType::Frame | ObuType::FrameHeader => {
                if is_keyframe(&mut cursor).map_err(DetectGopStartError::Av1ParserError)? {
                    keyframe_found = true;
                }
            }
            _ => {
                // Skip other OBUs
            }
        }

        // Skip the OBU payload
        skip_obu(&mut cursor, obu_size).map_err(DetectGopStartError::Av1ParserError)?;
    }

    if keyframe_found && let Some(details) = video_encoding_details {
        Ok(GopStartDetection::StartOfGop(details))
    } else {
        Ok(GopStartDetection::NotStartOfGop)
    }
}

fn skip_obu<R: io::Read + io::Seek>(reader: &mut R, obu_size: u64) -> io::Result<()> {
    let offset = i64::try_from(obu_size).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("payload size exceeds seek limits: {err}"),
        )
    })?;

    reader.seek(SeekFrom::Current(offset))?;
    Ok(())
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
                assert_eq!(details.codec_string, "av01");
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
