use std::io::Cursor;

use cros_codecs::codec::h265::parser::{
    Nalu as H265Nalu, NaluType as H265NaluType, Parser as H265Parser,
};
use h264_reader::nal::{self, Nal as _};
use itertools::Itertools as _;
use re_span::Span;
use saturating_cast::SaturatingCast as _;

use super::{SampleMetadata, VideoDataDescription, VideoLoadError};
use crate::demux::{
    ChromaSubsamplingModes, SampleMetadataState, SamplesStatistics, VideoDeliveryMethod,
    VideoEncodingDetails,
};
use crate::h264::encoding_details_from_h264_sps;
use crate::h265::encoding_details_from_h265_sps;
use crate::nalu::ANNEXB_NAL_START_CODE;
use crate::{StableIndexDeque, Time, Timescale};

impl VideoDataDescription {
    pub fn load_mp4(
        bytes: &[u8],
        debug_name: &str,
        source_id: re_tuid::Tuid,
    ) -> Result<Self, VideoLoadError> {
        re_tracing::profile_function!();
        let mp4 = {
            re_tracing::profile_scope!("Mp4::read_bytes");
            re_mp4::Mp4::read_bytes(bytes)?
        };

        let mp4_tracks = mp4.tracks().iter().map(|(k, t)| (*k, t.kind)).collect();

        let track = mp4
            .tracks()
            .values()
            .find(|t| t.kind == Some(re_mp4::TrackKind::Video))
            .ok_or(VideoLoadError::NoVideoTrack)?;

        let stsd = track.trak(&mp4).mdia.minf.stbl.stsd.clone();

        let timescale = Timescale::new(track.timescale);
        let mut samples =
            StableIndexDeque::<SampleMetadataState>::with_capacity(track.samples.len());
        let mut keyframe_indices = Vec::new();

        {
            re_tracing::profile_scope!("copy samples & build gops");

            for sample in &track.samples {
                if sample.is_sync {
                    keyframe_indices.push(samples.next_index());
                }

                let decode_timestamp = Time::new(sample.decode_timestamp);
                let presentation_timestamp = Time::new(sample.composition_timestamp);
                let duration = Time::new(sample.duration.saturating_cast());

                let byte_span = Span {
                    start: sample.offset as u32,
                    len: sample.size as u32,
                };

                samples.push_back(SampleMetadataState::Present(SampleMetadata {
                    is_sync: sample.is_sync,
                    frame_nr: 0, // filled in after the loop
                    decode_timestamp,
                    presentation_timestamp,
                    duration: Some(duration),
                    source_id,
                    byte_span,
                }));
            }
        }

        // Generate data for `test_latest_sample_index_at_presentation_timestamp` test.
        if false {
            re_log::info!(
                "pts: {:?}",
                samples
                    .iter()
                    .take(50)
                    .filter_map(|s| Some(s.sample()?.presentation_timestamp.0))
                    .collect::<Vec<_>>()
            );
            re_log::info!(
                "dts: {:?}",
                samples
                    .iter()
                    .take(50)
                    .filter_map(|s| Some(s.sample()?.decode_timestamp.0))
                    .collect::<Vec<_>>()
            );
        }

        {
            re_tracing::profile_scope!("Sanity-check samples");
            let mut samples_are_in_decode_order = true;
            for (a, b) in samples
                .iter()
                .tuple_windows::<(&SampleMetadataState, &SampleMetadataState)>()
                .filter_map(|(a, b)| Some((a.sample()?, b.sample()?)))
            {
                samples_are_in_decode_order &= a.decode_timestamp <= b.decode_timestamp;
            }
            if !samples_are_in_decode_order {
                re_log::warn!(
                    "Video samples are NOT in decode order. This implies either invalid video data or a bug in parsing the mp4."
                );
            }
        }

        {
            re_tracing::profile_scope!("Calculate frame numbers");
            let mut samples_sorted_by_pts = samples
                .iter_mut()
                .filter_map(|f| f.sample_mut())
                .collect::<Vec<_>>();
            samples_sorted_by_pts.sort_by_key(|s| s.presentation_timestamp);
            for (frame_nr, sample) in samples_sorted_by_pts.into_iter().enumerate() {
                sample.frame_nr = frame_nr as u32;
            }
        }

        let samples_statistics = SamplesStatistics::new(&samples);

        let codec = match &stsd.contents {
            re_mp4::StsdBoxContent::Av01(_) => crate::VideoCodec::AV1,
            re_mp4::StsdBoxContent::Avc1(_) => crate::VideoCodec::H264,
            re_mp4::StsdBoxContent::Hvc1(_) | re_mp4::StsdBoxContent::Hev1(_) => {
                crate::VideoCodec::H265
            }
            re_mp4::StsdBoxContent::Vp08(_) => crate::VideoCodec::VP8,
            re_mp4::StsdBoxContent::Vp09(_) => crate::VideoCodec::VP9,
            _ => {
                return Err(VideoLoadError::UnsupportedCodec(unknown_codec_fourcc(
                    &mp4, track,
                )));
            }
        };

        let video_data_description = Self {
            codec,
            encoding_details: Some(codec_details_from_stds(track, stsd)?),
            timescale: Some(timescale),
            delivery_method: VideoDeliveryMethod::Static {
                duration: Time::new(track.duration.saturating_cast()),
            },
            samples_statistics,
            keyframe_indices,
            samples,
            mp4_tracks,
        };

        #[expect(clippy::panic)]
        if cfg!(debug_assertions)
            && let Err(err) = video_data_description.sanity_check()
        {
            panic!("VideoDataDescription sanity check for {debug_name} failed: {err}");
        }

        Ok(video_data_description)
    }
}

fn unknown_codec_fourcc(mp4: &re_mp4::Mp4, track: &re_mp4::Track) -> re_mp4::FourCC {
    let stsd = &track.trak(mp4).mdia.minf.stbl.stsd;
    match &stsd.contents {
        re_mp4::StsdBoxContent::Unknown(four_cc) => *four_cc,
        _ => Default::default(),
    }
}

fn codec_details_from_stds(
    track: &re_mp4::Track,
    stsd: re_mp4::StsdBox,
) -> Result<VideoEncodingDetails, VideoLoadError> {
    // For AVC we don't have to rely on the stsd box, since we can parse the SPS directly.
    // re_mp4 doesn't have a full SPS parser, so almost certainly we're getting more information out this way,
    // also this means that we have less divergence with the video streaming case.
    match &stsd.contents {
        re_mp4::StsdBoxContent::Avc1(avcc_box) => {
            if let Some(sps_nal) = avcc_box.avcc.sequence_parameter_sets.first() {
                let complete = true;
                let sps_nal = nal::RefNal::new(sps_nal.bytes.as_slice(), &[], complete);

                return nal::sps::SeqParameterSet::from_bits(sps_nal.rbsp_bits())
                    .and_then(|sps| encoding_details_from_h264_sps(&sps))
                    .map_err(VideoLoadError::SpsParsingError)
                    .map(|details| VideoEncodingDetails {
                        stsd: Some(stsd),
                        ..details
                    });
            }
        }
        re_mp4::StsdBoxContent::Hev1(hvc1_box) | re_mp4::StsdBoxContent::Hvc1(hvc1_box) => {
            let hvcc = &*hvc1_box.hvcc;

            for array in &hvcc.arrays {
                if let Ok(nalu_type) = H265NaluType::try_from(array.nal_unit_type as u32)
                    && matches!(nalu_type, H265NaluType::SpsNut)
                {
                    for nal in &array.nalus {
                        let mut annexb =
                            Vec::with_capacity(ANNEXB_NAL_START_CODE.len() + nal.size as usize);
                        annexb.extend_from_slice(ANNEXB_NAL_START_CODE);
                        annexb.extend_from_slice(&nal.data);

                        let mut parser = H265Parser::default();
                        let mut rdr = Cursor::new(annexb.as_slice());

                        if let Ok(nalu) = H265Nalu::next(&mut rdr) {
                            let sps_ref = parser
                                .parse_sps(&nalu)
                                .map_err(|_err| VideoLoadError::NoVideoTrack)?;
                            let details = encoding_details_from_h265_sps(sps_ref);

                            return Ok(VideoEncodingDetails {
                                stsd: Some(stsd.clone()),
                                ..details
                            });
                        }
                    }
                }
            }
        }
        _ => {}
    }

    Ok(VideoEncodingDetails {
        codec_string: stsd
            .contents
            .codec_string()
            .ok_or(VideoLoadError::UnableToDetermineCodecString)?,
        coded_dimensions: [track.width, track.height],
        bit_depth: stsd.contents.bit_depth(),
        chroma_subsampling: subsampling_mode(&stsd),
        stsd: Some(stsd),
    })
}

fn subsampling_mode(stsd: &re_mp4::StsdBox) -> Option<ChromaSubsamplingModes> {
    match &stsd.contents {
        re_mp4::StsdBoxContent::Av01(av01_box) => {
            if av01_box.av1c.monochrome {
                Some(ChromaSubsamplingModes::Monochrome)
            } else {
                // These are boolean options, see https://aomediacodec.github.io/av1-isobmff/#av1codecconfigurationbox-semantics
                // For spec of meaning see https://aomediacodec.github.io/av1-spec/av1-spec.pdf#page=131
                match (
                    av01_box.av1c.chroma_subsampling_x != 0,
                    av01_box.av1c.chroma_subsampling_y != 0,
                ) {
                    (true, true) => Some(ChromaSubsamplingModes::Yuv420), // May also be monochrome, but we already checked for that.
                    (true, false) => Some(ChromaSubsamplingModes::Yuv422),
                    (false, true) => None, // Downsampling in Y but not in X is unheard of!
                    (false, false) => Some(ChromaSubsamplingModes::Yuv444),
                }
            }
        }

        re_mp4::StsdBoxContent::Avc1(_) => {
            // We can only reach this point if there was no SPS.
            // In this case, this is an unplayable MP4 file anyways!
            None
        }

        re_mp4::StsdBoxContent::Hvc1(_) | re_mp4::StsdBoxContent::Hev1(_) => {
            // TODO(andreas): Parse HVC1/HEV1 SPS
            None
        }

        re_mp4::StsdBoxContent::Vp08(vp08_box) => {
            // Via https://www.ffmpeg.org/doxygen/4.3/vpcc_8c_source.html#l00116
            // enum VPX_CHROMA_SUBSAMPLING
            // {
            //     VPX_SUBSAMPLING_420_VERTICAL = 0,
            //     VPX_SUBSAMPLING_420_COLLOCATED_WITH_LUMA = 1,
            //     VPX_SUBSAMPLING_422 = 2,
            //     VPX_SUBSAMPLING_444 = 3,
            // };
            match vp08_box.vpcc.chroma_subsampling {
                0 | 1 => Some(ChromaSubsamplingModes::Yuv420),
                2 => Some(ChromaSubsamplingModes::Yuv422),
                3 => Some(ChromaSubsamplingModes::Yuv444),
                _ => None, // Unknown mode.
            }
        }
        re_mp4::StsdBoxContent::Vp09(vp09_box) => {
            // As above!
            match vp09_box.vpcc.chroma_subsampling {
                0 | 1 => Some(ChromaSubsamplingModes::Yuv420),
                2 => Some(ChromaSubsamplingModes::Yuv422),
                3 => Some(ChromaSubsamplingModes::Yuv444),
                _ => None, // Unknown mode.
            }
        }

        re_mp4::StsdBoxContent::Mp4a(_)
        | re_mp4::StsdBoxContent::Tx3g(_)
        | re_mp4::StsdBoxContent::Unknown(_) => None,
    }
}
