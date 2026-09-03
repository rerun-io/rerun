//! General H.264 utilities.

use h264_reader::nal::{self, Nal as _};

use crate::nalu::{
    ANNEXB_NAL_START_CODE, AnnexBStreamState, AnnexBStreamWriteError,
    write_length_prefixed_nalus_to_annexb_stream,
};
use crate::{
    ChromaSubsamplingModes, Chunk, DetectGopStartError, GopStartDetection, VideoEncodingDetails,
};

/// Retrieve [`VideoEncodingDetails`] from a H.264 SPS.
pub fn encoding_details_from_h264_sps(sps: re_video_parsing::ParsedSps) -> VideoEncodingDetails {
    let info = sps.info;

    // Codec string defined by WebCodec points to various spec documents.
    // https://www.w3.org/TR/webcodecs-avc-codec-registration/#fully-qualified-codec-strings
    // Not having read those, this is what we use in `re_mp4` and it works fine.
    // Also as of writing, Claude 4 agrees and is able to nicely explain its meaning.
    let profile = info.profile_idc;
    let constraint = info.constraint_flags;
    let level = info.level_idc;
    let codec_string = format!("avc1.{profile:02X}{constraint:02X}{level:02X}");

    let chroma_subsampling = match info.chroma_format_idc {
        0 => Some(ChromaSubsamplingModes::Monochrome),
        1 => Some(ChromaSubsamplingModes::Yuv420),
        2 => Some(ChromaSubsamplingModes::Yuv422),
        3 => Some(ChromaSubsamplingModes::Yuv444),
        idc => {
            re_log::error_once!("Invalid chroma format in H264 SPS: {idc}");
            None
        }
    };

    VideoEncodingDetails {
        codec_string,
        coded_dimensions: info.pixel_dimensions,
        bit_depth: Some(info.bit_depth_chroma),
        chroma_subsampling,
        h264: Some(std::sync::Arc::new(sps)),
        stsd: None,
    }
}

/// What the SPS of a sample turned out to be.
enum SpsOutcome {
    /// The same NAL unit as the SPS the known encoding details were derived from.
    Unchanged,

    Parsed(VideoEncodingDetails),

    /// The reason the SPS couldn't be read.
    Failed(String),
}

/// Try to determine whether a frame chunk is the start of a closed GOP in an h264 Annex B encoded stream.
///
/// See [`crate::detect_gop_start`] for `known_details`.
pub fn detect_h264_annexb_gop(
    sample_data: &[u8],
    known_details: Option<&VideoEncodingDetails>,
) -> Result<GopStartDetection, DetectGopStartError> {
    let Ok(nal_ranges) = re_video_parsing::nal_ranges(sample_data) else {
        // Data without any NAL start code has no NAL units to inspect.
        return Ok(GopStartDetection::NotStartOfGop);
    };

    let known_sps_nal = known_details
        .and_then(|details| details.h264.as_ref())
        .map(|sps| sps.nal.as_slice());

    let mut sps_outcome: Option<SpsOutcome> = None;
    let mut idr_frame_found = false;

    for range in nal_ranges {
        let nal_bytes = &sample_data[range];
        let nal = nal::RefNal::new(nal_bytes, &[], true);
        let Ok(nal_header) = nal.header() else {
            continue;
        };

        match nal_header.nal_unit_type() {
            nal::UnitType::SeqParameterSet => {
                // Note that if we find several SPS, we'll always use the latest one.
                sps_outcome = Some(if known_sps_nal == Some(nal_bytes) {
                    // Encoders repeat the SPS in front of every IDR frame, so this is
                    // the common case for every sample but the first.
                    SpsOutcome::Unchanged
                } else {
                    match re_video_parsing::ParsedSps::new(nal_bytes)
                        .map(encoding_details_from_h264_sps)
                    {
                        Ok(coding_details) => {
                            // A bit too much string concatenation something that frequent, better to enable this only for debug builds.
                            if cfg!(debug_assertions) {
                                re_log::trace!(
                                    "Parsed SPS to coding details for video stream: {coding_details:?}"
                                );
                            }
                            SpsOutcome::Parsed(coding_details)
                        }
                        Err(sps_err) => SpsOutcome::Failed(format!(
                            "Failed reading SPS: {sps_err:?}" // NOLINT: h264 errors don't implement display
                        )),
                    }
                });
            }
            nal::UnitType::SliceLayerWithoutPartitioningIdr => {
                idr_frame_found = true;
            }
            _ => {}
        }

        // In case of SPS parsing failure keep going.
        // It's unlikely, but maybe there's another SPS in the chunk that succeeds parsing.
        if idr_frame_found
            && matches!(
                sps_outcome,
                Some(SpsOutcome::Unchanged | SpsOutcome::Parsed(_))
            )
        {
            break;
        }
    }

    // In theory it could happen that we got an SPS but no IDR frame.
    // Arguably we should preserve the information from the SPS, but practically it's not useful:
    // If we never hit an IDR frame, then we can't play the video and every IDR frame is supposed to have
    // the *same* SPS.
    match sps_outcome {
        Some(SpsOutcome::Parsed(decoding_details)) if idr_frame_found => {
            Ok(GopStartDetection::StartOfGop(decoding_details))
        }
        Some(SpsOutcome::Unchanged) if idr_frame_found => {
            Ok(GopStartDetection::StartOfGopSameEncoding)
        }
        Some(SpsOutcome::Failed(error_str)) => Err(
            DetectGopStartError::FailedToExtractEncodingDetails(error_str),
        ),
        _ => Ok(GopStartDetection::NotStartOfGop),
    }
}

/// Write an H.264 chunk to an Annex B stream without state tracking.
///
/// This is a fully re-entrant utility that allows explicit control over parameter set emission.
/// Typically you'd pass `chunk.is_sync` to emit parameter sets for IDR frames only.
pub fn write_avc_chunk_to_annexb(
    avcc: &re_mp4::Avc1Box,
    nalu_stream: &mut dyn std::io::Write,
    emit_parameter_sets: bool,
    chunk: &Chunk,
) -> Result<(), AnnexBStreamWriteError> {
    re_tracing::profile_function!();

    let avcc = &avcc.avcc;

    // Emit SPS & PPS parameter sets if requested
    if emit_parameter_sets {
        for sps in &avcc.sequence_parameter_sets {
            nalu_stream.write_all(ANNEXB_NAL_START_CODE)?;
            nalu_stream.write_all(&sps.bytes)?;
        }
        for pps in &avcc.picture_parameter_sets {
            nalu_stream.write_all(ANNEXB_NAL_START_CODE)?;
            nalu_stream.write_all(&pps.bytes)?;
        }
    }

    // Each NAL unit in mp4 is prefixed with a length prefix.
    // In Annex B this doesn't exist.
    let length_prefix_size = avcc.length_size_minus_one as usize + 1;

    write_length_prefixed_nalus_to_annexb_stream(nalu_stream, &chunk.data, length_prefix_size)
}

pub fn write_avc_chunk_to_nalu_stream(
    avcc: &re_mp4::Avc1Box,
    nalu_stream: &mut dyn std::io::Write,
    chunk: &Chunk,
    state: &mut AnnexBStreamState,
) -> Result<(), AnnexBStreamWriteError> {
    re_tracing::profile_function!();

    // We expect the stream of chunks to not have any SPS (Sequence Parameter Set) & PPS (Picture Parameter Set)
    // just as it is the case with MP4 data.
    // In order to have every IDR frame be able to be fully re-entrant, we need to prepend the SPS & PPS NAL units.
    // Otherwise the decoder is not able to get the necessary information about how the video stream is encoded.
    let emit_parameter_sets = chunk.is_sync && !state.previous_frame_was_idr;

    write_avc_chunk_to_annexb(avcc, nalu_stream, emit_parameter_sets, chunk)?;
    state.previous_frame_was_idr = emit_parameter_sets;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::{GopStartDetection, detect_h264_annexb_gop};
    use crate::{ChromaSubsamplingModes, DetectGopStartError, VideoEncodingDetails};

    #[test]
    fn test_detect_h264_annexb_gop() {
        // Example H.264 Annex B encoded data containing SPS and IDR frame. (ai generated)
        let sample_data = &[
            // SPS NAL unit
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80, // IDR frame NAL unit
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B,
            0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
        ];
        let GopStartDetection::StartOfGop(details) =
            detect_h264_annexb_gop(sample_data, None).unwrap()
        else {
            panic!("expected the sample to start a GOP");
        };
        assert_eq!(details.codec_string, "avc1.64000A");
        assert_eq!(details.coded_dimensions, [64, 64]);
        assert_eq!(details.bit_depth, Some(8));
        assert_eq!(
            details.chroma_subsampling,
            Some(ChromaSubsamplingModes::Yuv420)
        );
        assert!(details.stsd.is_none());

        let sps = details.h264.expect("SPS of the sample");
        // The SPS NAL unit is kept around so that decoders can recognize it by its bytes.
        assert_eq!(sps.nal, sample_data[4..29]);
        assert_eq!(
            sps.info,
            re_video_parsing::SpsInfo {
                profile_idc: 100,
                constraint_flags: 0,
                level_idc: 10,
                pixel_dimensions: [64, 64],
                coded_extent: [64, 64],
                chroma_format_idc: 1,
                bit_depth_luma: 8,
                bit_depth_chroma: 8,
                frames_only: true,
                max_num_ref_frames: 16,
                max_num_reorder_frames: 2,
            }
        );

        // Example H.264 Annex B encoded data containing broken SPS and IDR frame. (above example but messed with the SPS)
        let sample_data = &[
            // SPS NAL unit
            0x00, 0x00, 0x00, 0x01, 0x67, 0x00, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80, // IDR frame NAL unit
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B,
            0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
        ];
        let result = detect_h264_annexb_gop(sample_data, None);
        assert_eq!(
            result,
            Err(DetectGopStartError::FailedToExtractEncodingDetails(
                "Failed reading SPS: RbspReaderError(RemainingData)".to_owned()
            ))
        );

        // Garbage data, still annex b shaped. (ai generated)
        let sample_data = &[
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80,
        ];
        let result = detect_h264_annexb_gop(sample_data, None);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));

        // Garbage data, no detectable nalu units.
        let sample_data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A];
        let result = detect_h264_annexb_gop(sample_data, None);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));
    }

    /// Every keyframe of a stream repeats the SPS. Once the details are known, the
    /// repeats are recognized by their bytes and the details stay as they are.
    #[test]
    fn repeated_sps_keeps_the_known_encoding_details() {
        let sample_data = &[
            // SPS NAL unit
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80, // IDR frame NAL unit
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B,
            0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
        ];

        let GopStartDetection::StartOfGop(details) =
            detect_h264_annexb_gop(sample_data, None).unwrap()
        else {
            panic!("expected the sample to start a GOP");
        };

        assert_eq!(
            detect_h264_annexb_gop(sample_data, Some(&details)),
            Ok(GopStartDetection::StartOfGopSameEncoding)
        );

        // An SPS that isn't the known one is read as usual.
        let other_details = VideoEncodingDetails {
            h264: None,
            ..details
        };
        assert!(matches!(
            detect_h264_annexb_gop(sample_data, Some(&other_details)),
            Ok(GopStartDetection::StartOfGop(_))
        ));
    }
}
