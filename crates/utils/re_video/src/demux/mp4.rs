#![allow(clippy::map_err_ignore)]

use itertools::Itertools as _;

use super::{GroupOfPictures, SampleMetadata, VideoDataDescription, VideoLoadError};

use crate::{
    StableIndexDeque, Time, Timescale,
    demux::{ChromaSubsamplingModes, SamplesStatistics, VideoEncodingDetails},
};

impl VideoDataDescription {
    pub fn load_mp4(bytes: &[u8]) -> Result<Self, VideoLoadError> {
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
        let duration = Time::new(track.duration as i64);
        let mut samples = StableIndexDeque::<SampleMetadata>::with_capacity(track.samples.len());
        let mut gops = StableIndexDeque::<GroupOfPictures>::new();
        let mut gop_sample_start_index = 0;

        {
            re_tracing::profile_scope!("copy samples & build gops");

            for sample in &track.samples {
                if sample.is_sync && !samples.is_empty() {
                    let start = samples[gop_sample_start_index].decode_timestamp;
                    let sample_range = gop_sample_start_index..samples.next_index();
                    gops.push_back(GroupOfPictures {
                        decode_start_time: start,
                        sample_range,
                    });
                    gop_sample_start_index = samples.next_index();
                }

                let decode_timestamp = Time::new(sample.decode_timestamp);
                let presentation_timestamp = Time::new(sample.composition_timestamp);
                let duration = Time::new(sample.duration as i64);

                let byte_offset = sample.offset as u32;
                let byte_length = sample.size as u32;

                samples.push_back(SampleMetadata {
                    is_sync: sample.is_sync,
                    frame_nr: 0, // filled in after the loop
                    decode_timestamp,
                    presentation_timestamp,
                    duration: Some(duration),
                    // There's only a single buffer, which is the raw mp4 video data.
                    buffer_index: 0,
                    byte_offset,
                    byte_length,
                });
            }
        }

        // Generate data for `test_latest_sample_index_at_presentation_timestamp` test.
        if false {
            re_log::info!(
                "pts: {:?}",
                samples
                    .iter()
                    .take(50)
                    .map(|s| s.presentation_timestamp.0)
                    .collect::<Vec<_>>()
            );
            re_log::info!(
                "dts: {:?}",
                samples
                    .iter()
                    .take(50)
                    .map(|s| s.decode_timestamp.0)
                    .collect::<Vec<_>>()
            );
        }

        // Append the last GOP if there are any samples left:
        if !samples.is_empty() {
            let start = samples[gop_sample_start_index].decode_timestamp;
            let sample_range = gop_sample_start_index..samples.next_index();
            gops.push_back(GroupOfPictures {
                decode_start_time: start,
                sample_range,
            });
        }

        {
            re_tracing::profile_scope!("Sanity-check samples");
            let mut samples_are_in_decode_order = true;
            for (a, b) in samples
                .iter()
                .tuple_windows::<(&SampleMetadata, &SampleMetadata)>()
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
            let mut samples_sorted_by_pts = samples.iter_mut().collect::<Vec<_>>();
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

        Ok(Self {
            codec,
            encoding_details: Some(codec_details_from_stds(track, stsd)?),
            timescale: Some(timescale),
            duration: Some(duration),
            samples_statistics,
            gops,
            samples,
            mp4_tracks,
        })
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
    Ok(VideoEncodingDetails {
        codec_string: stsd
            .contents
            .codec_string()
            .ok_or(VideoLoadError::UnableToDetermineCodecString)?,
        coded_dimensions: [track.width, track.height],
        bit_depth: stsd.contents.bit_depth(),
        is_monochrome: is_monochrome(&stsd),
        chroma_subsampling: subsampling_mode(&stsd),
        stsd: Some(stsd),
    })
}

fn subsampling_mode(stsd: &re_mp4::StsdBox) -> Option<ChromaSubsamplingModes> {
    match &stsd.contents {
        re_mp4::StsdBoxContent::Av01(av01_box) => {
            // These are boolean options, see https://aomediacodec.github.io/av1-isobmff/#av1codecconfigurationbox-semantics
            match (
                av01_box.av1c.chroma_subsampling_x != 0,
                av01_box.av1c.chroma_subsampling_y != 0,
            ) {
                (true, true) => Some(ChromaSubsamplingModes::Yuv420), // May also be monochrome.
                (true, false) => Some(ChromaSubsamplingModes::Yuv422),
                (false, true) => None, // Downsampling in Y but not in X is unheard of!
                // Either that or monochrome.
                // See https://aomediacodec.github.io/av1-spec/av1-spec.pdf#page=131
                (false, false) => Some(ChromaSubsamplingModes::Yuv444),
            }
        }

        re_mp4::StsdBoxContent::Avc1(_) => {
            // TODO move SPS parsing from ffmpeg to here.
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

fn is_monochrome(stsd: &re_mp4::StsdBox) -> Option<bool> {
    match &stsd.contents {
        re_mp4::StsdBoxContent::Av01(av01_box) => Some(av01_box.av1c.monochrome),
        re_mp4::StsdBoxContent::Avc1(_)
        | re_mp4::StsdBoxContent::Hvc1(_)
        | re_mp4::StsdBoxContent::Hev1(_) => {
            // It should be possible to extract this from the picture parameter set.
            None
        }
        re_mp4::StsdBoxContent::Vp08(_) | re_mp4::StsdBoxContent::Vp09(_) => {
            // Similar to AVC/HEVC, this information is likely accessible through SPS.
            None
        }

        re_mp4::StsdBoxContent::Mp4a(_)
        | re_mp4::StsdBoxContent::Tx3g(_)
        | re_mp4::StsdBoxContent::Unknown(_) => None,
    }
}
