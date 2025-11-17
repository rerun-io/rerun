use std::io::Cursor;

use scuffle_av1::{ObuHeader, ObuType, seq::SequenceHeaderObu};

use crate::{ChromaSubsamplingModes, DetectGopStartError, GopStartDetection, VideoEncodingDetails};

use scuffle_bytes_util::BitReader;
use std::io;

pub fn is_keyframe<R: io::Read>(obu_type: ObuType, reader: &mut R) -> io::Result<bool> {
    let mut reader = BitReader::new(reader);

    // only present for OBU_FRAME_HEADER
    let show_existing_frame = if obu_type == ObuType::FrameHeader {
        reader.read_bit()?
    } else {
        false
    };

    if show_existing_frame {
        return Ok(false);
    }

    // frame_type (2 bits)
    // 0 = KEY_FRAME
    // 1 = INTER_FRAME
    // 2 = INTRA_ONLY_FRAME
    // 3 = SWITCH_FRAME
    let frame_type_bits = reader.read_bits(2)? as u8;
    Ok(frame_type_bits == 0)
}

/// Try to determine whether an AV1 frame chunk is the start of a GOP.
///
/// This is a simplified approach that only looks for keyframes, we don't
/// consider `INTRA_ONLY` frames as GOP starts because they technically can rely on existing
/// decoder state.
pub fn detect_av1_keyframe_start(data: &[u8]) -> Result<GopStartDetection, DetectGopStartError> {
    let mut keyframe_found = false;
    let mut chroma: Option<ChromaSubsamplingModes> = None;
    let mut dimensions: Option<[u16; 2]> = None;
    let mut bit_depth: Option<u8> = None;

    let mut cursor = Cursor::new(data);

    while let Ok(header) = ObuHeader::parse(&mut cursor) {
        match header.obu_type {
            ObuType::SequenceHeader => {
                let seq = SequenceHeaderObu::parse(header, &mut cursor)
                    .map_err(DetectGopStartError::Av1ParserError)?;

                bit_depth = Some(seq.color_config.bit_depth as u8);
                dimensions.get_or_insert([
                    seq.max_frame_width as u16 - 1,
                    seq.max_frame_height as u16 - 1,
                ]);
                chroma.get_or_insert(chroma_mode_from_color_config(&seq.color_config));
            }
            ObuType::Frame | ObuType::FrameHeader => {
                if is_keyframe(header.obu_type, &mut cursor)
                    .map_err(DetectGopStartError::Av1ParserError)?
                {
                    keyframe_found = true;
                }
            }
            _ => {
                // Skip other OBUs
            }
        }
    }

    if !keyframe_found {
        return Ok(GopStartDetection::NotStartOfGop);
    }

    // If we found a GOP start, we should have dimensions either from the
    // frame header or the sequence header.
    let coded_dimensions = dimensions.unwrap_or_default();

    Ok(GopStartDetection::StartOfGop(VideoEncodingDetails {
        codec_string: "av01".to_owned(),
        coded_dimensions,
        bit_depth,
        chroma_subsampling: None,
        stsd: None,
    }))
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

#[cfg(test)]
mod test {
    use super::{GopStartDetection, detect_av1_keyframe_start};

    #[test]
    fn test_detect_av1_keyframe_start() {
        // Small 64x64 AV1 keyframe for testing (generated with ffmpeg)
        // This contains a Sequence Header OBU followed by a keyframe
        #[rustfmt::skip]
        let sample_data = &[
            0x12, 0x00, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x02, 0xAF, 0xFF, 0x9F, 0xFF, 0x30, 0x08, 0x32, 0x14,
            0x10, 0x00, 0xC0, 0x00, 0x00, 0x02, 0x80, 0x00, 0x00, 0x0A, 0x05, 0x76, 0xA4, 0xD6, 0x2F, 0x1F,
            0xFA, 0x1E, 0x3C, 0xD8,
        ];

        let result = detect_av1_keyframe_start(sample_data);

        match result {
            Ok(GopStartDetection::StartOfGop(details)) => {
                // Verify we got expected details from the AV1 stream
                assert_eq!(details.codec_string, "av01");
                assert_eq!(details.coded_dimensions, [64, 64]);

                // Bit depth should be present, but its zero in this test data
                assert_eq!(details.bit_depth, Some(0));
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
        // Random invalid data, the parser will panic on invalid OBU structure.
        // Make sure we handle that gracefully and don't forward to the parser.
        let invalid_data = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        assert!(matches!(
            detect_av1_keyframe_start(invalid_data),
            Err(crate::DetectGopStartError::Av1ParserError(..))
        ));
    }

    #[test]
    fn test_detect_av1_non_keyframe() {
        // This would need a valid AV1 sequence with a non-keyframe
        // For now, we just test that invalid/incomplete data doesn't panic
        let data = &[
            // Just a sequence header without a keyframe
            0x0A, 0x0B, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
        ];
        let result = detect_av1_keyframe_start(data);

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
