use std::io::Cursor;

use symphonia::core::audio::sample::Sample as _;
use symphonia::core::audio::{Channels, Position};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::{AudioBuffer, ChannelLayout, ChannelPosition};

/// Refuse to decode past this many bytes of PCM, so a long recording cannot exhaust memory.
///
/// 512 MiB is about 23 minutes of 48 kHz stereo.
const MAX_DECODED_BYTES: usize = 512 * 1024 * 1024;

#[derive(thiserror::Error, Debug, Clone)]
pub enum AudioDecodeError {
    #[error("Unrecognized or unsupported audio container: {0}")]
    UnsupportedFormat(String),

    #[error("The file contains no audio track")]
    NoAudioTrack,

    #[error("Unsupported audio codec: {0}")]
    UnsupportedCodec(String),

    #[error("Failed to decode audio: {0}")]
    Decode(String),

    #[error("The audio track is empty")]
    Empty,

    #[error(
        "The decoded audio would take more than {} MiB of memory; longer recordings are not supported yet",
        MAX_DECODED_BYTES / (1024 * 1024)
    )]
    TooLong,
}

/// Decodes a whole encoded audio file into interleaved `f32` PCM.
///
/// All channels are kept; callers that require stereo must downmix them before playback.
///
/// The result starts at the first decoded frame: container timing (track start time,
/// packet timestamps) is ignored, so a track with a leading gap or an edit list
/// comes out shifted.
/// Encoder delay and padding are trimmed by symphonia's gapless mode.
///
/// `media_type` (e.g. `audio/aac`) is a hint for the container probe.
/// Raw ADTS AAC streams have no reliable magic bytes, so the hint matters for them.
/// A wrong hint is harmless for formats with magic bytes, since the probe checks those first.
pub fn decode(bytes: &[u8], media_type: Option<&str>) -> Result<AudioBuffer, AudioDecodeError> {
    re_tracing::profile_function!();

    let mut hint = Hint::new();
    if let Some(media_type) = media_type {
        hint.mime_type(media_type);
        if let Some(extension) = extension_for_media_type(media_type) {
            hint.with_extension(extension);
        }
    }

    let source = Box::new(Cursor::new(bytes));
    let stream = MediaSourceStream::new(source, Default::default());

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|err| AudioDecodeError::UnsupportedFormat(err.to_string()))?;

    let (mut track_id, mut decoder) = make_decoder(format.as_ref())?;

    let mut sample_rate = None;
    let mut channels: Option<Channels> = None;
    let mut samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => {
                // The track list changed (e.g. a chained Ogg stream): pick the audio track again.
                (track_id, decoder) = make_decoder(format.as_ref())?;
                continue;
            }
            Err(err) => return Err(AudioDecodeError::Decode(err.to_string())),
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(err)) => {
                re_log::debug_once!("Skipping undecodable audio packet: {err}");
                continue;
            }
            Err(err) => return Err(AudioDecodeError::Decode(err.to_string())),
        };

        let spec = decoded.spec();
        match (sample_rate, &channels) {
            (None, _) | (_, None) => {
                sample_rate = Some(spec.rate());
                channels = Some(spec.channels().clone());
            }
            (Some(rate), Some(channels)) => {
                if rate != spec.rate() || channels != spec.channels() {
                    return Err(AudioDecodeError::Decode(
                        "The sample rate or channel layout changed mid-stream".to_owned(),
                    ));
                }
            }
        }

        let start = samples.len();
        let end = start + decoded.samples_interleaved();
        if MAX_DECODED_BYTES < end * size_of::<f32>() {
            return Err(AudioDecodeError::TooLong);
        }
        samples.resize(end, f32::MID);
        decoded.copy_to_slice_interleaved(&mut samples[start..end]);
    }

    let (Some(sample_rate), Some(channels)) = (sample_rate, channels) else {
        return Err(AudioDecodeError::Empty);
    };
    let num_channels = channels.count() as u32;
    if samples.is_empty() || num_channels == 0 {
        return Err(AudioDecodeError::Empty);
    }

    Ok(AudioBuffer {
        sample_rate,
        num_channels,
        layout: ChannelLayout::from_symphonia(&channels),
        samples,
    })
}

impl ChannelLayout {
    /// Maps symphonia's speaker positions onto the coarse [`ChannelPosition`]s.
    fn from_symphonia(channels: &Channels) -> Self {
        const FRONT_LEFT: Position = Position::FRONT_LEFT.union(Position::FRONT_LEFT_WIDE);
        const FRONT_RIGHT: Position = Position::FRONT_RIGHT.union(Position::FRONT_RIGHT_WIDE);
        const LEFT: Position = Position::REAR_LEFT
            .union(Position::SIDE_LEFT)
            .union(Position::FRONT_LEFT_CENTER)
            .union(Position::TOP_FRONT_LEFT)
            .union(Position::TOP_REAR_LEFT)
            .union(Position::TOP_SIDE_LEFT)
            .union(Position::BOTTOM_FRONT_LEFT);
        const RIGHT: Position = Position::REAR_RIGHT
            .union(Position::SIDE_RIGHT)
            .union(Position::FRONT_RIGHT_CENTER)
            .union(Position::TOP_FRONT_RIGHT)
            .union(Position::TOP_REAR_RIGHT)
            .union(Position::TOP_SIDE_RIGHT)
            .union(Position::BOTTOM_FRONT_RIGHT);
        const LOW_FREQUENCY: Position = Position::LFE1.union(Position::LFE2);

        let Channels::Positioned(positions) = channels else {
            return Self::Unknown;
        };
        let positions = positions
            .iter()
            .map(|position| {
                if FRONT_LEFT.contains(position) {
                    ChannelPosition::FrontLeft
                } else if FRONT_RIGHT.contains(position) {
                    ChannelPosition::FrontRight
                } else if LEFT.contains(position) {
                    ChannelPosition::Left
                } else if RIGHT.contains(position) {
                    ChannelPosition::Right
                } else if LOW_FREQUENCY.contains(position) {
                    ChannelPosition::LowFrequency
                } else {
                    ChannelPosition::Center
                }
            })
            .collect();
        Self::Positioned(positions)
    }
}

/// Selects the default audio track and creates a decoder for it.
fn make_decoder(
    format: &(dyn FormatReader + '_),
) -> Result<(u32, Box<dyn AudioDecoder>), AudioDecodeError> {
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(AudioDecodeError::NoAudioTrack)?;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or(AudioDecodeError::NoAudioTrack)?;

    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|err| AudioDecodeError::UnsupportedCodec(err.to_string()))?;

    Ok((track.id, decoder))
}

fn extension_for_media_type(media_type: &str) -> Option<&'static str> {
    match media_type {
        "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => Some("wav"),
        "audio/aac" | "audio/aacp" => Some("aac"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/ogg" => Some("ogg"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAV: &[u8] = include_bytes!("../../../../tests/assets/audio/sine_440hz_2s.wav");

    fn toreador_song() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tests/assets/audio/toreador_song.aac"
        ))
        .expect("Missing test asset; is git LFS installed?")
    }

    fn assert_is_two_second_sine(buffer: &AudioBuffer, tolerance_secs: f64, peak_tolerance: f32) {
        assert_eq!(buffer.sample_rate, 16000);
        assert_eq!(buffer.num_channels, 1);
        assert!(
            (buffer.duration_secs() - 2.0).abs() < tolerance_secs,
            "duration was {}",
            buffer.duration_secs()
        );

        let peak = buffer.samples.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!((peak - 0.5).abs() <= peak_tolerance, "peak was {peak}");

        // Count zero crossings in the middle second: a 440 Hz sine has 880 per second.
        let middle = &buffer.samples[8000..24000];
        let crossings = middle
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        assert!((850..=910).contains(&crossings), "crossings: {crossings}");
    }

    #[test]
    fn decode_wav() {
        let buffer = decode(WAV, Some("audio/wav")).unwrap();
        assert_is_two_second_sine(&buffer, 1e-6, 0.01);
    }

    #[test]
    fn decode_wav_without_hint() {
        let buffer = decode(WAV, None).unwrap();
        assert_is_two_second_sine(&buffer, 1e-6, 0.01);
    }

    #[test]
    fn decode_wav_with_wrong_hint() {
        let buffer = decode(WAV, Some("audio/aac")).unwrap();
        assert_is_two_second_sine(&buffer, 1e-6, 0.01);
    }

    #[test]
    fn decode_aac_adts() {
        let buffer = decode(&toreador_song(), Some("audio/aac")).unwrap();
        assert_eq!(buffer.sample_rate, 44100);
        assert_eq!(buffer.num_channels, 2);
        assert!(
            (buffer.duration_secs() - 270.63).abs() < 0.1,
            "duration was {}",
            buffer.duration_secs()
        );

        let peak = buffer.samples.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!((0.5..=1.0).contains(&peak), "peak was {peak}");
    }

    #[test]
    fn channel_layout_of_5_1() {
        // Interleaved order follows the bit order: FL, FR, FC, LFE, RL, RR.
        let channels = Channels::Positioned(
            Position::FRONT_LEFT
                | Position::FRONT_RIGHT
                | Position::FRONT_CENTER
                | Position::LFE1
                | Position::REAR_LEFT
                | Position::REAR_RIGHT,
        );
        assert_eq!(
            ChannelLayout::from_symphonia(&channels),
            ChannelLayout::Positioned(vec![
                ChannelPosition::FrontLeft,
                ChannelPosition::FrontRight,
                ChannelPosition::Center,
                ChannelPosition::LowFrequency,
                ChannelPosition::Left,
                ChannelPosition::Right,
            ])
        );
        assert_eq!(
            ChannelLayout::from_symphonia(&Channels::Discrete(4)),
            ChannelLayout::Unknown
        );
    }

    #[test]
    fn decode_garbage_fails() {
        assert!(decode(b"definitely not audio", None).is_err());
        assert!(decode(&[], Some("audio/wav")).is_err());
    }
}
