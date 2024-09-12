#![allow(clippy::map_err_ignore)]

use super::{Config, Sample, Segment, Time, Timescale, VideoData, VideoLoadError};
use ::mp4;

use mp4::TrackKind;
use vec1::Vec1;

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

    let timescale = Timescale::new(video_track.timescale);
    let duration = Time::new(video_track.duration);
    let mut samples = Vec::<Sample>::new();
    let mut segments = Vec::<Segment>::new();
    let data = video_track.data.clone();

    for sample in &video_track.samples {
        if sample.is_sync {
            if let Ok(samples) = Vec1::try_from_vec(samples) {
                segments.push(Segment { samples });
            }
            samples = Vec::new();
        }

        let decode_timestamp = Time::new(sample.decode_timestamp);
        let composition_timestamp = Time::new(sample.composition_timestamp);
        let duration = Time::new(sample.duration);

        let byte_offset = sample.offset as u32;
        let byte_length = sample.size as u32;

        samples.push(Sample {
            decode_timestamp,
            composition_timestamp,
            duration,
            byte_offset,
            byte_length,
        });
    }

    if let Ok(samples) = Vec1::try_from_vec(samples) {
        segments.push(Segment { samples });
    }

    Ok(VideoData {
        config,
        timescale,
        duration,
        segments,
        data,
    })
}
