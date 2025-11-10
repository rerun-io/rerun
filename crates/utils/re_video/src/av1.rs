use cros_codecs::codec::av1::parser::{FrameHeaderObu, FrameType, ObuAction, ParsedObu, Parser};

use crate::{DetectGopStartError, GopStartDetection, VideoEncodingDetails};

/// Try to determine whether an AV1 frame chunk is the start of a GOP.
/// This is a simplified approach that only looks for keyframes, `INTRA_ONLY` frames are not considered GOP starts.
pub fn detect_av1_keyframe_start(data: &[u8]) -> Result<GopStartDetection, DetectGopStartError> {
    let mut parser = Parser::default();
    let mut offset = 0usize;

    let mut gop_found = false;
    let mut dimensions: Option<[u16; 2]> = None;
    let mut bit_depth: Option<u8> = None;

    while offset < data.len() {
        let action = parser
            .read_obu(&data[offset..])
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
