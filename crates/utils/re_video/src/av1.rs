use cros_codecs::codec::av1::parser::{
    ColorConfig, FrameHeaderObu, FrameType, ObuAction, ParsedObu, Parser,
};

use crate::{ChromaSubsamplingModes, DetectGopStartError, GopStartDetection, VideoEncodingDetails};

/// Try to determine whether an AV1 frame chunk is the start of a GOP.
///
/// This is a simplified approach that only looks for keyframes, we don't
/// consider `INTRA_ONLY` frames as GOP starts because they technically can rely on existing
/// decoder state.
pub fn detect_av1_keyframe_start(data: &[u8]) -> Result<GopStartDetection, DetectGopStartError> {
    let mut parser = Parser::default();
    let mut offset = 0usize;

    let mut gop_found = false;
    let mut chroma: Option<ChromaSubsamplingModes> = None;
    let mut dimensions: Option<[u16; 2]> = None;
    let mut bit_depth: Option<u8> = None;

    while offset < data.len() {
        let slice = &data[offset..];
        if slice.is_empty() {
            // No more data to parse
            break;
        }

        // the parser panics if the OBU is malformed, we want to avoid that
        // so lets make sure the reserved bit is zero (as per spec)
        let obu_reserved_1bit = (slice[0] >> 7) & 0x01;
        if obu_reserved_1bit != 0 {
            return Err(DetectGopStartError::Av1ParserError(
                "Malformed OBU: reserved bit not zero".to_owned(),
            ));
        }

        let action = parser
            .read_obu(slice)
            .map_err(DetectGopStartError::Av1ParserError)?;

        match action {
            ObuAction::Drop(num_bytes) => {
                offset += num_bytes as usize;
            }
            ObuAction::Process(obu) => {
                let bytes_used = obu.bytes_used;
                let parsed = parser
                    .parse_obu(obu)
                    .map_err(DetectGopStartError::Av1ParserError)?;

                offset += bytes_used;

                match parsed {
                    ParsedObu::Frame(frame) => {
                        let header = &frame.header;
                        if is_gop_start(header) {
                            gop_found = true;
                            dimensions =
                                Some([header.frame_width as u16, header.frame_height as u16]);
                        }
                    }
                    ParsedObu::FrameHeader(header) => {
                        if is_gop_start(&header) {
                            gop_found = true;
                            dimensions =
                                Some([header.frame_width as u16, header.frame_height as u16]);
                        }
                    }
                    ParsedObu::SequenceHeader(sequence) => {
                        bit_depth = Some(sequence.bit_depth as u8);

                        // Only use dimensions from sequence header if we don't have more
                        // precise frame-based dimensions yet.
                        dimensions.get_or_insert([
                            sequence.max_frame_width_minus_1,
                            sequence.max_frame_height_minus_1,
                        ]);
                        chroma.get_or_insert(chroma_mode_from_color_config(&sequence.color_config));
                    }
                    _ => {
                        // ignore other OBUs
                    }
                }
            }
        }
    }

    if !gop_found {
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
fn is_gop_start(header: &FrameHeaderObu) -> bool {
    header.frame_type == FrameType::KeyFrame && header.show_frame && !header.show_existing_frame
}

#[inline]
fn chroma_mode_from_color_config(config: &ColorConfig) -> ChromaSubsamplingModes {
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
            Ok(GopStartDetection::NotStartOfGop) => {
                panic!("Expected to detect GOP start but got NotStartOfGop");
            }
            Err(e) => {
                panic!("Failed to parse valid AV1 data: {e:?}");
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
