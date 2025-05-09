#![allow(clippy::map_err_ignore)]

use super::{Config, GroupOfPictures, Sample, VideoData, VideoLoadError};

use crate::{Time, Timescale, demux::SamplesStatistics};

impl VideoData {
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

        let description = track
            .raw_codec_config(&mp4)
            .ok_or_else(|| VideoLoadError::UnsupportedCodec(unknown_codec_fourcc(&mp4, track)))?;

        let coded_height = track.height;
        let coded_width = track.width;

        let config = Config {
            stsd,
            description,
            coded_height,
            coded_width,
        };

        let timescale = Timescale::new(track.timescale);
        let duration = Time::new(track.duration as i64);
        let mut samples = Vec::<Sample>::new();
        let mut gops = Vec::<GroupOfPictures>::new();
        let mut gop_sample_start_index = 0;

        {
            re_tracing::profile_scope!("copy samples & build gops");

            for (sample_idx, sample) in track.samples.iter().enumerate() {
                if sample.is_sync && !samples.is_empty() {
                    let start = samples[gop_sample_start_index].decode_timestamp;
                    let sample_range = gop_sample_start_index as u32..samples.len() as u32;
                    gops.push(GroupOfPictures {
                        decode_start_time: start,
                        sample_range,
                    });
                    gop_sample_start_index = samples.len();
                }

                let decode_timestamp = Time::new(sample.decode_timestamp);
                let presentation_timestamp = Time::new(sample.composition_timestamp);
                let duration = Time::new(sample.duration as i64);

                let byte_offset = sample.offset as u32;
                let byte_length = sample.size as u32;

                samples.push(Sample {
                    is_sync: sample.is_sync,
                    sample_idx,
                    frame_nr: 0, // filled in after the loop
                    decode_timestamp,
                    presentation_timestamp,
                    duration,
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
            let sample_range = gop_sample_start_index as u32..samples.len() as u32;
            gops.push(GroupOfPictures {
                decode_start_time: start,
                sample_range,
            });
        }

        {
            re_tracing::profile_scope!("Sanity-check samples");
            let mut samples_are_in_decode_order = true;
            for window in samples.windows(2) {
                samples_are_in_decode_order &=
                    window[0].decode_timestamp <= window[1].decode_timestamp;
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
                sample.frame_nr = frame_nr;
            }
        }

        let samples_statistics = SamplesStatistics::new(&samples);

        Ok(Self {
            config,
            timescale,
            duration,
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
