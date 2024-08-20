#![allow(clippy::map_err_ignore)]

use super::{Config, Sample, Segment, VideoData, VideoLoadError};
use ::mp4;

use mp4::TrackType;
use std::io::Cursor;
use std::io::Write as _;

pub fn load_mp4(bytes: &[u8]) -> Result<VideoData, VideoLoadError> {
    let mut mp4 = ::mp4::Mp4Reader::read_header(Cursor::new(bytes), bytes.len() as u64)?;

    let video_track = mp4
        .tracks()
        .values()
        .find(|t| t.track_type().ok() == Some(TrackType::Video))
        .ok_or_else(|| VideoLoadError::NoVideoTrack)?;
    let track_id = video_track.track_id();
    let num_samples = video_track.sample_count();

    let (codec, description);
    if let Some(::mp4::Av01Box { av1c, .. }) = &video_track.trak.mdia.minf.stbl.stsd.av01 {
        let profile = av1c.profile;
        let level = av1c.level;
        let tier = if av1c.tier == 0 { "M" } else { "H" };
        let bit_depth = av1c.bit_depth;

        codec = format!("av01.{profile}.{level:02}{tier}.{bit_depth:02}");
        description = write_box_without_header(av1c)?;
    } else {
        panic!("todo: support other codecs");
    }

    let timescale = video_track.trak.mdia.mdhd.timescale as u64;
    let duration = video_track.trak.mdia.mdhd.duration;
    let coded_height = video_track.height();
    let coded_width = video_track.width();

    let config = Config {
        codec,
        description,
        coded_height,
        coded_width,
    };

    let mut time_offset = None;
    let mut samples = Vec::<Sample>::new();
    let mut segments = Vec::<Segment>::new();
    let mut data = Vec::<u8>::new();

    for sample_idx in 0..num_samples {
        let sample = mp4
            .read_sample(track_id, sample_idx + 1)
            .map_err(|_err| VideoLoadError::InvalidSamples)?
            .ok_or_else(|| VideoLoadError::InvalidSamples)?;

        if sample.is_sync && !samples.is_empty() {
            segments.push(Segment {
                timestamp: samples[0].timestamp,
                samples,
            });
            samples = Vec::new();
        }

        let time_offset = *time_offset.get_or_insert(sample.start_time);
        let timestamp = sample.start_time - time_offset;
        let byte_offset = data.len() as u32;
        let byte_length = sample.bytes.len() as u32;
        data.write_all(&sample.bytes).expect("oom");

        samples.push(Sample {
            timestamp,
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
        timescale,
        duration,
        segments,
    })
}

fn write_box_without_header<Box: for<'a> mp4::WriteBox<&'a mut Vec<u8>>>(
    b: &Box,
) -> Result<Vec<u8>, VideoLoadError> {
    let mut out = Vec::new();
    b.write_box(&mut out)
        .map_err(|_| VideoLoadError::InvalidConfigFormat)?;
    let mut cursor = Cursor::new(&out);
    let _ = mp4::BoxHeader::read(&mut cursor).map_err(|_| VideoLoadError::InvalidConfigFormat)?;
    let config_start = cursor.position() as usize;
    let _ = out.splice(0..config_start, std::iter::empty());
    Ok(out)
}
