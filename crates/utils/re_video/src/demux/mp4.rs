#![allow(clippy::map_err_ignore)]

use itertools::Itertools as _;

use super::{GroupOfPictures, Sample, VideoDataDescription, VideoLoadError};

use crate::{StableIndexDeque, Time, Timescale, demux::SamplesStatistics};

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
        let mut samples = StableIndexDeque::<Sample>::with_capacity(track.samples.len());
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

                samples.push_back(Sample {
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
            for window in samples.iter().tuple_windows::<(&Sample, &Sample)>() {
                samples_are_in_decode_order &=
                    window.0.decode_timestamp <= window.1.decode_timestamp;
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
            re_mp4::StsdBoxContent::Av01(_) => crate::VideoCodec::Av1,
            re_mp4::StsdBoxContent::Avc1(_) => crate::VideoCodec::H264,
            re_mp4::StsdBoxContent::Hvc1(_) | re_mp4::StsdBoxContent::Hev1(_) => {
                crate::VideoCodec::H265
            }
            re_mp4::StsdBoxContent::Vp08(_) => crate::VideoCodec::Vp8,
            re_mp4::StsdBoxContent::Vp09(_) => crate::VideoCodec::Vp9,
            _ => {
                return Err(VideoLoadError::UnsupportedCodec(unknown_codec_fourcc(
                    &mp4, track,
                )));
            }
        };

        Ok(Self {
            codec,
            stsd: Some(stsd),
            coded_dimensions: Some([track.width, track.height]),
            timescale,
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
