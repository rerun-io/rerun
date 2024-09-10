#![allow(clippy::map_err_ignore)]

use crate::TimeMs;

use super::{Config, Sample, Segment, VideoData, VideoLoadError};
use ::mp4;

use mp4::TrackKind;

pub fn load_mp4(bytes: &[u8]) -> Result<VideoData, VideoLoadError> {
    let mp4 = ::mp4::read(bytes)?;

    let video_track = mp4
        .tracks()
        .values()
        .find(|t| t.kind == TrackKind::Video)
        .ok_or_else(|| VideoLoadError::NoVideoTrack)?;

    let codec = video_track
        .codec_string(&mp4)
        .ok_or_else(|| VideoLoadError::UnsupportedCodec)?;
    let codec = if codec.starts_with("vp08") {
        "vp8".to_owned()
    } else {
        codec
    };

    let description = video_track
        .raw_codec_config(&mp4)
        .ok_or_else(|| VideoLoadError::UnsupportedCodec)?;

    let coded_height = video_track.height;
    let coded_width = video_track.width;

    let config = Config {
        codec,
        description,
        coded_height,
        coded_width,
    };

    let duration = TimeMs::new(video_track.duration_ms());
    let mut samples = Vec::<Sample>::new();
    let mut segments = Vec::<Segment>::new();
    let data = video_track.data.clone();

    for sample in &video_track.samples {
        if sample.is_sync && !samples.is_empty() {
            segments.push(Segment {
                timestamp: samples[0].timestamp,
                samples,
            });
            samples = Vec::new();
        }

        let timestamp = TimeMs::new(sample.timestamp_ms());
        let duration = TimeMs::new(sample.duration_ms());

        let byte_offset = sample.offset as u32;
        let byte_length = sample.size as u32;

        samples.push(Sample {
            timestamp,
            duration,
            byte_offset,
            byte_length,
        });
    }

    if !samples.is_empty() {
        segments.push(Segment {
            timestamp: samples[0].timestamp,
            samples,
        });
    }

    Ok(VideoData {
        config,
        data,
        duration,
        segments,
    })
}
