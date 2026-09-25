//! Mixes any number of PCM streams into one interleaved stereo output.
//!
//! Terminology:
//! - A *sample* is one number: the amplitude of one channel at one instant, in `[-1, 1]`.
//! - A *frame* is one sample per all channels at one instant. 48 kHz audio has 48000 frames per second.
//! - *Volume* is what the caller asks for: a linear multiplier on the samples, `0.0` silent, `1.0` unchanged.
//! - *Gain* is the multiplier actually applied right now. It ramps toward the requested volume
//!   (or toward `0.0` when stopping) over a few milliseconds, because jumping it instantly clicks.
//!   The ramp is far too short to hear as a fade.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{AUDIBLE_SPEEDS, AudioBuffer, StreamId, StreamRequest};

/// Output device settings. Sources at other sample rates are resampled on the fly.
pub const OUTPUT_SAMPLE_RATE: u32 = 48_000;
pub const OUTPUT_CHANNELS: usize = 2;

/// If the caller's playhead and the stream's own cursor drift apart by more than this,
/// the cursor jumps to the playhead. Below it, the audio clock is trusted so playback stays smooth.
const RESYNC_THRESHOLD_SECS: f64 = 0.1;

/// How long gain changes (start, stop, jumps, volume) take.
///
/// A stream that starts or jumps mid-clip, or is cut off mid-clip, would otherwise begin or end
/// on a sample far from zero. That step is heard as a pop, so the gain ramps instead.
const GAIN_RAMP_SECS: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamMode {
    /// Keep playing for as long as each committed update requests it.
    WhileRequested,

    /// Keep playing until reaching the end of the buffer.
    Once,
}

struct PendingRequest {
    request: StreamRequest,
    mode: StreamMode,
}

/// One stream being mixed, with its own read position driven by the audio clock.
struct Stream {
    buffer: Arc<AudioBuffer>,

    /// Read position in source frames.
    cursor: f64,

    /// Playback speed multiplier, where `1.0` is real time.
    speed: f32,

    /// Multiplier applied to the samples right now. Ramps toward `target_gain` to avoid clicks.
    gain: f32,

    /// The requested volume, or `0.0` while muted, stopping, or stale.
    target_gain: f32,

    mode: StreamMode,

    /// Whether the stream was omitted from the latest committed update.
    stale: bool,
}

/// Mixes multiple streams of [`AudioBuffer`]s into one interleaved stereo output.
///
/// Used by [`crate::AudioPlayer`]: the output device's callback calls [`Self::fill`]
/// to get the next chunk of samples, while the caller keeps streams alive with
/// [`Self::request`] from the UI thread, then commits or discards the complete update.
#[derive(Default)]
pub struct Mixer {
    streams: BTreeMap<StreamId, Stream>,

    /// The streams requested by the current, uncommitted update.
    pending_requests: BTreeMap<StreamId, PendingRequest>,
}

impl Mixer {
    /// Stage a stream for the current update.
    ///
    /// If the same stream is requested more than once, the last request wins.
    pub fn request(&mut self, request: StreamRequest) {
        self.stage(request, StreamMode::WhileRequested);
    }

    fn stage(&mut self, request: StreamRequest, mode: StreamMode) {
        let non_finite: Vec<&str> = [
            ("position_secs", request.position_secs.is_finite()),
            ("speed", request.speed.is_finite()),
            ("volume", request.volume.is_finite()),
        ]
        .into_iter()
        .filter_map(|(name, is_finite)| (!is_finite).then_some(name))
        .collect();
        if !non_finite.is_empty() {
            re_log::debug_once!(
                "Ignoring audio request with non-finite {}",
                non_finite.join(", ")
            );
            return;
        }

        self.pending_requests
            .insert(request.id, PendingRequest { request, mode });
    }

    fn apply_request(&mut self, pending: PendingRequest) {
        let PendingRequest {
            request:
                StreamRequest {
                    id,
                    buffer,
                    position_secs,
                    speed,
                    volume,
                },
            mode,
        } = pending;

        let audible = AUDIBLE_SPEEDS.contains(&speed) && 0.0 < volume;
        let target_gain = if audible { volume } else { 0.0 };
        let target_cursor = position_secs * buffer.sample_rate as f64;

        let stream = self.streams.entry(id).or_insert_with(|| Stream {
            buffer: buffer.clone(),
            cursor: target_cursor,
            speed,
            gain: 0.0,
            target_gain,
            mode,
            stale: false,
        });

        // The stream's cursor follows the audio clock, so a caller's playhead that merely
        // jitters frame to frame is ignored: jumping to it would stutter. Only a new buffer,
        // or a real seek (drift past the threshold), moves the cursor. The gain restarts
        // from zero so the jump ramps up instead of popping.
        let same_buffer = Arc::ptr_eq(&stream.buffer, &buffer);
        let drift_secs = (stream.cursor - target_cursor).abs() / buffer.sample_rate as f64;
        if !same_buffer || RESYNC_THRESHOLD_SECS < drift_secs {
            stream.buffer = buffer;
            stream.cursor = target_cursor;
            stream.gain = 0.0;
        }

        stream.speed = speed;
        stream.target_gain = target_gain;
        stream.mode = mode;
        stream.stale = false;
    }

    /// Play a buffer from start to end once, at normal speed and full volume.
    ///
    /// The one-shot starts on [`Self::commit_requests`] and then lives until it ends,
    /// replacing any stream with the same `id`.
    pub fn play_once(&mut self, id: StreamId, buffer: Arc<AudioBuffer>) {
        self.stage(
            StreamRequest {
                id,
                buffer,
                position_secs: 0.0,
                speed: 1.0,
                volume: 1.0,
            },
            StreamMode::Once,
        );
    }

    /// Commit the staged requests as the complete desired set of persistent streams.
    ///
    /// Persistent streams that were not requested ramp down to silence within
    /// a few milliseconds and are then dropped.
    pub fn commit_requests(&mut self) {
        for (id, stream) in &mut self.streams {
            if stream.mode == StreamMode::WhileRequested && !self.pending_requests.contains_key(id)
            {
                stream.stale = true;
                stream.target_gain = 0.0;
            }
        }

        let pending_requests = std::mem::take(&mut self.pending_requests);
        for request in pending_requests.into_values() {
            self.apply_request(request);
        }
    }

    /// Discard all requests staged by the current update.
    pub fn discard_requests(&mut self) {
        self.pending_requests.clear();
    }

    /// Overwrites `out` with the next `out.len() / 2` frames of the mix,
    /// as interleaved stereo (`[left, right, left, right, …]`) at [`OUTPUT_SAMPLE_RATE`].
    ///
    /// `out` can be any length; each stream advances by as many frames as were filled.
    pub fn fill(&mut self, out: &mut [f32]) {
        re_tracing::profile_function!();

        out.fill(0.0);

        let gain_step = 1.0 / (GAIN_RAMP_SECS * OUTPUT_SAMPLE_RATE as f32);

        self.streams.retain(|_, stream| {
            stream.mix_into(out, gain_step);

            let ended = stream.buffer.num_frames() as f64 <= stream.cursor;
            let silent = stream.gain <= 0.0 && stream.target_gain <= 0.0;
            !(ended || (stream.stale && silent))
        });
    }
}

impl Stream {
    fn mix_into(&mut self, out: &mut [f32], gain_step: f32) {
        let Self {
            buffer,
            cursor,
            speed,
            gain,
            target_gain,
            mode: _,
            stale: _,
        } = self;

        let num_frames = buffer.num_frames();
        let num_channels = buffer.num_channels as usize;
        if num_frames == 0 || num_channels == 0 {
            return;
        }

        let cursor_step = *speed as f64 * buffer.sample_rate as f64 / OUTPUT_SAMPLE_RATE as f64;
        if cursor_step == 1.0 {
            // Same rate as the output: snap to whole frames, so the interpolator
            // lands exactly on source samples and returns them unchanged.
            *cursor = cursor.round();
        }
        let (left_channel, right_channel): (u32, u32) =
            if num_channels == 1 { (0, 0) } else { (0, 1) };

        for frame in out.chunks_exact_mut(OUTPUT_CHANNELS) {
            if *gain < *target_gain {
                *gain = (*gain + gain_step).min(*target_gain);
            } else if *target_gain < *gain {
                *gain = (*gain - gain_step).max(*target_gain);
            }

            if 0.0 < *gain
                && let Some(left) = buffer.sample_interpolated(*cursor, left_channel)
                && let Some(right) = buffer.sample_interpolated(*cursor, right_channel)
            {
                frame[0] += *gain * left;
                frame[1] += *gain * right;
            }

            // The cursor follows the audio clock no matter the gain, so a muted or
            // silenced stream keeps its place in the clip instead of pausing.
            *cursor += cursor_step;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChannelLayout;

    /// Frames per [`Mixer::fill`] call, like a typical device callback.
    const FILL_FRAMES: usize = 1024;

    fn ramp_buffer(sample_rate: u32, num_frames: usize) -> Arc<AudioBuffer> {
        Arc::new(AudioBuffer {
            sample_rate,
            num_channels: 1,
            layout: ChannelLayout::Unknown,
            samples: (0..num_frames).map(|i| i as f32).collect(),
        })
    }

    fn request(id: StreamId, buffer: &Arc<AudioBuffer>, position_secs: f64) -> StreamRequest {
        StreamRequest {
            id,
            buffer: buffer.clone(),
            position_secs,
            speed: 1.0,
            volume: 1.0,
        }
    }

    #[test]
    fn cursor_follows_audio_clock_within_threshold_and_jumps_beyond() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize * 10);
        let mut mixer = Mixer::default();

        mixer.request(request(StreamId(1), &buffer, 1.0));
        mixer.commit_requests();
        let mut out = vec![0.0; OUTPUT_CHANNELS * 4800];
        mixer.fill(&mut out);
        let cursor_after = mixer.streams[&StreamId(1)].cursor;
        assert_eq!(cursor_after, OUTPUT_SAMPLE_RATE as f64 + 4800.0);

        // Small drift: cursor is trusted.
        mixer.request(request(StreamId(1), &buffer, 1.15));
        mixer.commit_requests();
        assert_eq!(mixer.streams[&StreamId(1)].cursor, cursor_after);

        // Large drift: cursor jumps.
        mixer.request(request(StreamId(1), &buffer, 5.0));
        mixer.commit_requests();
        assert_eq!(
            mixer.streams[&StreamId(1)].cursor,
            5.0 * OUTPUT_SAMPLE_RATE as f64
        );
        assert_eq!(mixer.streams[&StreamId(1)].gain, 0.0);
    }

    #[test]
    fn stale_streams_are_removed() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize);
        let mut mixer = Mixer::default();
        mixer.request(request(StreamId(1), &buffer, 0.0));
        mixer.commit_requests();

        let mut out = vec![0.0; OUTPUT_CHANNELS * FILL_FRAMES];
        mixer.fill(&mut out);
        assert!(mixer.streams.contains_key(&StreamId(1)));
        assert!(out.iter().any(|s| *s != 0.0));

        mixer.commit_requests();
        mixer.fill(&mut out);
        assert!(!mixer.streams.contains_key(&StreamId(1)));
    }

    #[test]
    fn latest_request_for_a_stream_wins() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize * 10);
        let mut mixer = Mixer::default();

        mixer.request(request(StreamId(1), &buffer, 1.0));
        mixer.request(request(StreamId(1), &buffer, 2.0));
        mixer.commit_requests();

        assert_eq!(
            mixer.streams[&StreamId(1)].cursor,
            2.0 * OUTPUT_SAMPLE_RATE as f64
        );
    }

    #[test]
    fn discarded_requests_do_not_change_streams() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize * 10);
        let mut mixer = Mixer::default();
        mixer.request(request(StreamId(1), &buffer, 1.0));
        mixer.commit_requests();

        mixer.request(request(StreamId(1), &buffer, 5.0));
        mixer.discard_requests();

        assert_eq!(
            mixer.streams[&StreamId(1)].cursor,
            OUTPUT_SAMPLE_RATE as f64
        );
        assert!(!mixer.streams[&StreamId(1)].stale);
    }

    #[test]
    fn out_of_range_speed_is_silent_but_keeps_stream() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize);
        let mut mixer = Mixer::default();
        let mut req = request(StreamId(1), &buffer, 0.5);
        req.speed = 10.0;
        mixer.request(req);
        mixer.commit_requests();

        let mut out = vec![0.0; OUTPUT_CHANNELS * FILL_FRAMES];
        mixer.fill(&mut out);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(mixer.streams.contains_key(&StreamId(1)));
        assert_eq!(
            mixer.streams[&StreamId(1)].cursor,
            0.5 * OUTPUT_SAMPLE_RATE as f64 + 10.0 * FILL_FRAMES as f64
        );
    }

    #[test]
    fn muted_stream_is_silent_but_keeps_advancing() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize);
        let mut mixer = Mixer::default();
        let mut req = request(StreamId(1), &buffer, 0.0);
        req.volume = 0.0;
        mixer.request(req);
        mixer.commit_requests();

        let mut out = vec![0.0; OUTPUT_CHANNELS * FILL_FRAMES];
        mixer.fill(&mut out);
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(mixer.streams[&StreamId(1)].cursor, FILL_FRAMES as f64);
    }

    #[test]
    fn last_frame_is_played() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, 1);
        let mut mixer = Mixer::default();
        mixer.request(request(StreamId(1), &buffer, 0.0));
        mixer.commit_requests();
        mixer.streams.get_mut(&StreamId(1)).unwrap().gain = 1.0;

        // Silence in `ramp_buffer` is a zero sample, so put a nonzero one there instead.
        let buffer = Arc::new(AudioBuffer {
            sample_rate: OUTPUT_SAMPLE_RATE,
            num_channels: 1,
            layout: ChannelLayout::Unknown,
            samples: vec![1.0],
        });
        mixer.streams.get_mut(&StreamId(1)).unwrap().buffer = buffer;

        let mut out = vec![0.0; OUTPUT_CHANNELS * 4];
        mixer.fill(&mut out);
        assert_eq!(out[0], 1.0);
        assert_eq!(out[1], 1.0);
        assert!(out[2..].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn omitted_stream_ramps_down_and_keeps_advancing() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize);
        let mut mixer = Mixer::default();
        mixer.request(request(StreamId(1), &buffer, 0.0));
        mixer.commit_requests();
        mixer.streams.get_mut(&StreamId(1)).unwrap().gain = 1.0;
        mixer.commit_requests();

        let mut out = vec![0.0; OUTPUT_CHANNELS * 100];
        mixer.fill(&mut out);
        let stream = &mixer.streams[&StreamId(1)];
        assert!(0.0 < stream.gain && stream.gain < 1.0);
        assert_eq!(stream.cursor, 100.0);

        let mut out = vec![0.0; OUTPUT_CHANNELS * FILL_FRAMES];
        mixer.fill(&mut out);
        assert!(!mixer.streams.contains_key(&StreamId(1)));
    }

    #[test]
    fn one_shot_plays_to_the_end_without_requests() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, FILL_FRAMES + 10);
        let mut mixer = Mixer::default();
        mixer.play_once(StreamId(1), buffer);
        assert!(mixer.streams.is_empty(), "one-shot is only staged");
        mixer.commit_requests();

        let mut out = vec![0.0; OUTPUT_CHANNELS * FILL_FRAMES];
        mixer.fill(&mut out);
        assert!(out.iter().any(|s| *s != 0.0));
        assert_eq!(mixer.streams.len(), 1, "still has 10 frames to go");

        mixer.commit_requests();
        mixer.fill(&mut out);
        assert!(mixer.streams.is_empty(), "finished and removed");
    }

    #[test]
    fn discarded_one_shot_never_starts() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, FILL_FRAMES);
        let mut mixer = Mixer::default();

        mixer.play_once(StreamId(1), buffer);
        mixer.discard_requests();
        mixer.commit_requests();

        assert!(mixer.streams.is_empty());
    }

    #[test]
    fn same_rate_playback_is_sample_exact() {
        let buffer = ramp_buffer(OUTPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE as usize);
        let mut mixer = Mixer::default();
        mixer.request(request(
            StreamId(1),
            &buffer,
            10.4 / OUTPUT_SAMPLE_RATE as f64,
        ));
        mixer.commit_requests();
        mixer.streams.get_mut(&StreamId(1)).unwrap().gain = 1.0;

        let mut out = vec![0.0; OUTPUT_CHANNELS * FILL_FRAMES];
        mixer.fill(&mut out);
        for (i, frame) in out.chunks_exact(OUTPUT_CHANNELS).enumerate() {
            assert_eq!(frame[0], (10 + i) as f32);
        }
        assert_eq!(
            mixer.streams[&StreamId(1)].cursor,
            (10 + FILL_FRAMES) as f64
        );
    }

    #[test]
    fn resamples_and_upmixes_mono() {
        // 24 kHz mono source played at 48 kHz stereo: cursor advances half a frame per output frame.
        let buffer = ramp_buffer(24_000, 24_000);
        let mut mixer = Mixer::default();
        mixer.request(request(StreamId(1), &buffer, 0.0));
        mixer.commit_requests();

        let mut out = vec![0.0; OUTPUT_CHANNELS * 4800];
        mixer.fill(&mut out);
        assert_eq!(mixer.streams[&StreamId(1)].cursor, 2400.0);

        // After the gain ramp, left and right are equal and increase by 0.5 per frame.
        let frames: Vec<_> = out.chunks_exact(2).collect();
        let a = frames[4000];
        let b = frames[4001];
        assert_eq!(a[0], a[1]);
        assert!((b[0] - a[0] - 0.5).abs() < 1e-3, "{a:?} {b:?}");
    }
}
