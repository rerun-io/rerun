//! General H.264 utilities.

use h264_reader::annexb::AnnexBReader;
use h264_reader::nal::{self, Nal as _};
use h264_reader::push::NalInterest;
use h264_reader::rbsp::BitReaderError;

use crate::nalu::{
    ANNEXB_NAL_START_CODE, AnnexBStreamState, AnnexBStreamWriteError,
    write_length_prefixed_nalus_to_annexb_stream,
};
use crate::{
    ChromaSubsamplingModes, Chunk, DetectGopStartError, GopStartDetection, VideoEncodingDetails,
};

/// Retrieve [`VideoEncodingDetails`] from a H.264 SPS.
fn encoding_details_from_h264_sps(
    sps: &nal::sps::SeqParameterSet,
) -> Result<VideoEncodingDetails, nal::sps::SpsError> {
    let bit_depth = sps.chroma_info.bit_depth_chroma_minus8 + 8;

    // Codec string defined by WebCodec points to various spec documents.
    // https://www.w3.org/TR/webcodecs-avc-codec-registration/#fully-qualified-codec-strings
    // Not having read those, this is what we use in `re_mp4` and it works fine.
    // Also as of writing, Claude 4 agrees and is able to nicely explain its meaning.
    let profile = u8::from(sps.profile_idc);
    let constraint = u8::from(sps.constraint_flags);
    let level = sps.level_idc;
    let codec_string = format!("avc1.{profile:02X}{constraint:02X}{level:02X}");

    // Calculating the dimensions of the frame in pixels from the SPS is quite complicated
    // as it has to take into account cropping and concepts like macro block sizes.
    // Luckily h264-reader has a utility for this!
    let coded_dimensions = sps.pixel_dimensions()?;
    let chroma_subsampling = match sps.chroma_info.chroma_format {
        nal::sps::ChromaFormat::Monochrome => Some(ChromaSubsamplingModes::Monochrome),
        nal::sps::ChromaFormat::YUV420 => Some(ChromaSubsamplingModes::Yuv420),
        nal::sps::ChromaFormat::YUV422 => Some(ChromaSubsamplingModes::Yuv422),
        nal::sps::ChromaFormat::YUV444 => Some(ChromaSubsamplingModes::Yuv444),
        nal::sps::ChromaFormat::Invalid(_) => {
            re_log::error_once!(
                "Invalid chroma format in H264 SPS: {:?}",
                sps.chroma_info.chroma_format
            );
            None
        }
    };

    // Absent when the SPS carries no VUI bitstream restrictions, in which case a decoder falls
    // back to whatever its level allows.
    let max_num_reorder_frames = sps
        .vui_parameters
        .as_ref()
        .and_then(|vui| vui.bitstream_restrictions.as_ref())
        .map(|restrictions| restrictions.max_num_reorder_frames);

    Ok(VideoEncodingDetails {
        codec_string,
        coded_dimensions: [coded_dimensions.0 as _, coded_dimensions.1 as _],
        bit_depth: Some(bit_depth),
        chroma_subsampling,
        max_num_reorder_frames,
        stsd: None,
    })
}

/// Retrieve [`VideoEncodingDetails`] from a single H.264 SPS NAL unit, without start code or length prefix.
///
/// Zero bytes after the NAL unit are ignored.
/// A NAL unit never ends in a zero byte, so they can only be padding.
pub fn encoding_details_from_h264_sps_nal(
    mut sps_nal: &[u8],
) -> Result<VideoEncodingDetails, nal::sps::SpsError> {
    while let [rest @ .., 0x00] = sps_nal {
        sps_nal = rest;
    }

    // `RefNal::new` panics on empty input.
    if sps_nal.is_empty() {
        return Err(nal::sps::SpsError::RbspReaderError(
            BitReaderError::ReaderErrorFor(
                "nal_unit_header",
                std::io::ErrorKind::UnexpectedEof.into(),
            ),
        ));
    }

    let complete = true;
    let sps_nal = nal::RefNal::new(sps_nal, &[], complete);

    let sps = nal::sps::SeqParameterSet::from_bits(sps_nal.rbsp_bits())?;
    encoding_details_from_h264_sps(&sps)
}

#[derive(Default)]
struct H264GopDetectionState {
    coding_details_from_sps: Option<Result<VideoEncodingDetails, nal::sps::SpsError>>,
    idr_frame_found: bool,
}

impl h264_reader::push::AccumulatedNalHandler for H264GopDetectionState {
    fn nal(&mut self, nal: nal::RefNal<'_>) -> NalInterest {
        let Ok(nal_header) = nal.header() else {
            return NalInterest::Ignore;
        };
        let nal_unit_type = nal_header.nal_unit_type();

        if nal_unit_type == nal::UnitType::SeqParameterSet {
            if !nal.is_complete() {
                // Want full SPS, not just a partial one in order to extract the encoding details.
                return NalInterest::Buffer;
            }

            // Note that if we find several SPS, we'll always use the latest one.
            self.coding_details_from_sps = Some(
                match nal::sps::SeqParameterSet::from_bits(nal.rbsp_bits())
                    .and_then(|sps| encoding_details_from_h264_sps(&sps))
                {
                    Ok(coding_details) => {
                        // A bit too much string concatenation something that frequent, better to enable this only for debug builds.
                        if cfg!(debug_assertions) {
                            re_log::trace!(
                                "Parsed SPS to coding details for video stream: {coding_details:?}"
                            );
                        }
                        Ok(coding_details)
                    }
                    Err(sps_err) => Err(sps_err),
                },
            );
        } else if nal_unit_type == nal::UnitType::SliceLayerWithoutPartitioningIdr {
            self.idr_frame_found = true;
        }

        NalInterest::Ignore
    }
}

/// Try to determine whether a frame chunk is the start of a closed GOP in an h264 Annex B encoded stream.
pub fn detect_h264_annexb_gop(
    mut sample_data: &[u8],
) -> Result<GopStartDetection, DetectGopStartError> {
    let mut reader = AnnexBReader::accumulate(H264GopDetectionState::default());

    while !sample_data.is_empty() {
        // Don't parse everything at once.
        const MAX_CHUNK_SIZE: usize = 256;
        let chunk_size = MAX_CHUNK_SIZE.min(sample_data.len());

        reader.push(&sample_data[..chunk_size]);

        // In case of SPS parsing failure keep going.
        // It's unlikely, but maybe there's another SPS in the chunk that succeeds parsing.
        let handler = reader.nal_handler_ref();
        if handler.idr_frame_found && matches!(handler.coding_details_from_sps, Some(Ok(_))) {
            break;
        }

        sample_data = &sample_data[chunk_size..];
    }

    let handler = reader.into_nal_handler();
    match handler.coding_details_from_sps {
        Some(Ok(decoding_details)) => {
            if handler.idr_frame_found {
                Ok(GopStartDetection::StartOfGop(decoding_details))
            } else {
                // In theory it could happen that we got an SPS but no IDR frame.
                // Arguably we should preserve the information from the SPS, but practically it's not useful:
                // If we never hit an IDR frame, then we can't play the video and every IDR frame is supposed to have
                // the *same* SPS.
                Ok(GopStartDetection::NotStartOfGop)
            }
        }
        Some(Err(sps_err)) => Err(DetectGopStartError::FailedToExtractEncodingDetails(
            // relint: allow(debug_formatted_error): `h264_reader::nal::sps::SpsError` only implements `Debug`
            format!("Failed reading SPS: {sps_err:?}"),
        )),
        None => Ok(GopStartDetection::NotStartOfGop),
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
    use h264_reader::nal::sps::SpsError;
    use h264_reader::rbsp::BitReaderError;

    use super::{GopStartDetection, detect_h264_annexb_gop, encoding_details_from_h264_sps_nal};
    use crate::nalu::ANNEXB_NAL_START_CODE;
    use crate::{ChromaSubsamplingModes, DetectGopStartError, VideoEncodingDetails};

    const SPS_NAL: &[u8] = &[
        0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84, 0x00, 0x00, 0x03, 0x00, 0x04,
        0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11, 0x80,
    ];

    const IDR_NAL: &[u8] = &[
        0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A,
        0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
    ];

    fn sps_details() -> VideoEncodingDetails {
        VideoEncodingDetails {
            codec_string: "avc1.64000A".to_owned(),
            coded_dimensions: [64, 64],
            bit_depth: Some(8),
            chroma_subsampling: Some(ChromaSubsamplingModes::Yuv420),
            max_num_reorder_frames: Some(2),
            stsd: None,
        }
    }

    #[test]
    fn test_sps_nal_with_trailing_zero_bytes() {
        for num_trailing_zeros in 0..=4 {
            let mut sps_nal = SPS_NAL.to_vec();
            sps_nal.resize(SPS_NAL.len() + num_trailing_zeros, 0x00);

            match encoding_details_from_h264_sps_nal(&sps_nal) {
                Ok(details) => assert_eq!(details, sps_details()),
                Err(err) => panic!("{num_trailing_zeros} trailing zero bytes: {err:?}"),
            }
        }
    }

    #[test]
    fn test_empty_sps_nal() {
        for sps_nal in [&[][..], &[0x00, 0x00, 0x00]] {
            assert!(matches!(
                encoding_details_from_h264_sps_nal(sps_nal),
                Err(SpsError::RbspReaderError(BitReaderError::ReaderErrorFor(
                    "nal_unit_header",
                    _
                )))
            ));
        }
    }

    /// The decoders are handed the SPS of an `avcC` box as stored, followed by a start code.
    #[test]
    fn test_detect_h264_annexb_gop_with_trailing_zero_bytes() {
        for num_trailing_zeros in 0..=4 {
            let mut sample_data = ANNEXB_NAL_START_CODE.to_vec();
            sample_data.extend_from_slice(SPS_NAL);
            sample_data.resize(sample_data.len() + num_trailing_zeros, 0x00);
            sample_data.extend_from_slice(ANNEXB_NAL_START_CODE);
            sample_data.extend_from_slice(IDR_NAL);

            assert_eq!(
                detect_h264_annexb_gop(&sample_data),
                Ok(GopStartDetection::StartOfGop(sps_details())),
                "{num_trailing_zeros} trailing zero bytes"
            );
        }
    }

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
        let result = detect_h264_annexb_gop(sample_data);
        assert_eq!(
            result,
            Ok(GopStartDetection::StartOfGop(VideoEncodingDetails {
                codec_string: "avc1.64000A".to_owned(),
                coded_dimensions: [64, 64],
                bit_depth: Some(8),
                chroma_subsampling: Some(ChromaSubsamplingModes::Yuv420),
                max_num_reorder_frames: Some(2),
                stsd: None,
            }))
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
        let result = detect_h264_annexb_gop(sample_data);
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
        let result = detect_h264_annexb_gop(sample_data);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));

        // Garbage data, no detectable nalu units.
        let sample_data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A];
        let result = detect_h264_annexb_gop(sample_data);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));
    }
}
