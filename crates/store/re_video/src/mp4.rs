#![allow(clippy::map_err_ignore)]

use super::{Config, Sample, Segment, Time, Timescale, VideoData, VideoLoadError};

pub fn load_mp4(bytes: &[u8]) -> Result<VideoData, VideoLoadError> {
    let mp4 = re_mp4::read(bytes)?;

    let track = mp4
        .tracks()
        .values()
        .find(|t| t.kind == Some(re_mp4::TrackKind::Video))
        .ok_or_else(|| VideoLoadError::NoVideoTrack)?;

    let codec = track
        .codec_string(&mp4)
        .ok_or_else(|| VideoLoadError::UnsupportedCodec(unknown_codec_fourcc(&mp4, track)))?;
    let description = track
        .raw_codec_config(&mp4)
        .ok_or_else(|| VideoLoadError::UnsupportedCodec(unknown_codec_fourcc(&mp4, track)))?;

    let coded_height = track.height;
    let coded_width = track.width;

    let config = Config {
        codec,
        description,
        coded_height,
        coded_width,
    };

    let timescale = Timescale::new(track.timescale);
    let duration = Time::new(track.duration);
    let mut samples = Vec::<Sample>::new();
    let mut segments = Vec::<Segment>::new();
    let mut segment_sample_start_index = 0;
    let data = track.data.clone();

    for sample in &track.samples {
        if sample.is_sync && !samples.is_empty() {
            let start = samples[segment_sample_start_index].decode_timestamp;
            let sample_range = segment_sample_start_index as u32..samples.len() as u32;
            segments.push(Segment {
                start,
                sample_range,
            });
            segment_sample_start_index = samples.len();
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

    if !samples.is_empty() {
        let start = samples[segment_sample_start_index].decode_timestamp;
        let sample_range = segment_sample_start_index as u32..samples.len() as u32;
        segments.push(Segment {
            start,
            sample_range,
        });
    }

    Ok(VideoData {
        config,
        timescale,
        duration,
        segments,
        samples,
        data,
    })
}

fn unknown_codec_fourcc(mp4: &re_mp4::Mp4, track: &re_mp4::Track) -> re_mp4::FourCC {
    let stsd = &track.trak(mp4).mdia.minf.stbl.stsd;
    stsd.unknown.first().copied().unwrap_or_default()
}

/// Returns whether a buffer is MP4 video data.
///
/// From `infer` crate.
pub fn is_mp4(buf: &[u8]) -> bool {
    buf.len() > 11
        && (buf[4] == b'f' && buf[5] == b't' && buf[6] == b'y' && buf[7] == b'p')
        && ((buf[8] == b'a' && buf[9] == b'v' && buf[10] == b'c' && buf[11] == b'1')
            || (buf[8] == b'd' && buf[9] == b'a' && buf[10] == b's' && buf[11] == b'h')
            || (buf[8] == b'i' && buf[9] == b's' && buf[10] == b'o' && buf[11] == b'2')
            || (buf[8] == b'i' && buf[9] == b's' && buf[10] == b'o' && buf[11] == b'3')
            || (buf[8] == b'i' && buf[9] == b's' && buf[10] == b'o' && buf[11] == b'4')
            || (buf[8] == b'i' && buf[9] == b's' && buf[10] == b'o' && buf[11] == b'5')
            || (buf[8] == b'i' && buf[9] == b's' && buf[10] == b'o' && buf[11] == b'6')
            || (buf[8] == b'i' && buf[9] == b's' && buf[10] == b'o' && buf[11] == b'm')
            || (buf[8] == b'm' && buf[9] == b'm' && buf[10] == b'p' && buf[11] == b'4')
            || (buf[8] == b'm' && buf[9] == b'p' && buf[10] == b'4' && buf[11] == b'1')
            || (buf[8] == b'm' && buf[9] == b'p' && buf[10] == b'4' && buf[11] == b'2')
            || (buf[8] == b'm' && buf[9] == b'p' && buf[10] == b'4' && buf[11] == b'v')
            || (buf[8] == b'm' && buf[9] == b'p' && buf[10] == b'7' && buf[11] == b'1')
            || (buf[8] == b'M' && buf[9] == b'S' && buf[10] == b'N' && buf[11] == b'V')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'A' && buf[11] == b'S')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'S' && buf[11] == b'C')
            || (buf[8] == b'N' && buf[9] == b'S' && buf[10] == b'D' && buf[11] == b'C')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'S' && buf[11] == b'H')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'S' && buf[11] == b'M')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'S' && buf[11] == b'P')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'S' && buf[11] == b'S')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'X' && buf[11] == b'C')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'X' && buf[11] == b'H')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'X' && buf[11] == b'M')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'X' && buf[11] == b'P')
            || (buf[8] == b'N' && buf[9] == b'D' && buf[10] == b'X' && buf[11] == b'S')
            || (buf[8] == b'F' && buf[9] == b'4' && buf[10] == b'V' && buf[11] == b' ')
            || (buf[8] == b'F' && buf[9] == b'4' && buf[10] == b'P' && buf[11] == b' '))
}
