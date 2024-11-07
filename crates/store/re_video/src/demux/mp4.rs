#![allow(clippy::map_err_ignore)]

use super::{Config, GroupOfPictures, Sample, VideoData, VideoLoadError};

use crate::{demux::SamplesStatistics, Time, Timescale};

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
            .ok_or_else(|| VideoLoadError::NoVideoTrack)?;

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

            for sample in &track.samples {
                if sample.is_sync && !samples.is_empty() {
                    let start = samples[gop_sample_start_index].decode_timestamp;
                    let sample_range = gop_sample_start_index as u32..samples.len() as u32;
                    gops.push(GroupOfPictures {
                        decode_start_time: start,
                        sample_range,
                    });
                    gop_sample_start_index = samples.len();
                }

                let decode_timestamp = Time::new(sample.decode_timestamp as i64);
                let presentation_timestamp = Time::new(sample.composition_timestamp as i64);
                let duration = Time::new(sample.duration as i64);

                let byte_offset = sample.offset as u32;
                let byte_length = sample.size as u32;

                samples.push(Sample {
                    is_sync: sample.is_sync,
                    decode_timestamp,
                    presentation_timestamp,
                    duration,
                    byte_offset,
                    byte_length,
                });
            }
        }

        if !samples.is_empty() {
            let start = samples[gop_sample_start_index].decode_timestamp;
            let sample_range = gop_sample_start_index as u32..samples.len() as u32;
            gops.push(GroupOfPictures {
                decode_start_time: start,
                sample_range,
            });
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
