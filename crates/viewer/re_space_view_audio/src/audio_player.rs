use std::{collections::VecDeque, sync::Arc};

use parking_lot::Mutex;

use crate::visualizer_system::AudioEntry;

pub static AUDIO_PLAYER: once_cell::sync::Lazy<AudioPlayer> =
    once_cell::sync::Lazy::new(AudioPlayer::new);

/// Fade in and out over this many samples to avoid clicks.
const FADE_FRAMES: usize = 128; // TODO: what's a good value here?

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StereoFrame {
    pub left: f32,
    pub right: f32,
}

impl StereoFrame {
    #[inline]
    pub fn from_mono(mono: f32) -> Self {
        Self {
            left: mono,
            right: mono,
        }
    }

    #[inline]
    pub fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }
}

impl std::ops::Mul<f32> for StereoFrame {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            left: self.left * rhs,
            right: self.right * rhs,
        }
    }
}

impl std::ops::Mul<StereoFrame> for f32 {
    type Output = StereoFrame;

    #[inline]
    fn mul(self, rhs: StereoFrame) -> StereoFrame {
        rhs * self
    }
}

impl std::ops::Add for StereoFrame {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            left: self.left + rhs.left,
            right: self.right + rhs.right,
        }
    }
}

impl std::ops::AddAssign for StereoFrame {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

#[derive(Clone, Default)]
pub struct StereoAudio {
    pub frames: Vec<StereoFrame>,

    /// e.g. 44100 Hz
    pub frame_rate: f32,
}

impl StereoAudio {
    pub fn from_mono(frame_rate: f32, mono: impl ExactSizeIterator<Item = f32>) -> Self {
        Self {
            frames: mono.map(StereoFrame::from_mono).collect(),
            frame_rate,
        }
    }

    /// L,R,L,R,…
    pub fn from_stereo(frame_rate: f32, mut stereo: impl ExactSizeIterator<Item = f32>) -> Self {
        let num_frames = stereo.len() / 2;
        let mut frames = Vec::with_capacity(num_frames);
        for _ in 0..num_frames {
            let l = stereo.next().unwrap();
            let r = stereo.next().unwrap();
            frames.push(StereoFrame::new(l, r));
        }
        Self { frames, frame_rate }
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn resample(self, target_frame_rate: f32) -> Self {
        re_tracing::profile_function!();

        if self.frames.is_empty() {
            return self;
        }

        debug_assert!(0.0 < self.frame_rate);
        debug_assert!(0.0 < target_frame_rate);

        let factor = target_frame_rate / self.frame_rate;

        if (factor - 1.0).abs() < 0.01 {
            return self;
        }
        debug_assert!(0.0 < factor);
        debug_assert!(factor.is_finite());

        let out_n = (self.frames.len() as f32 * factor).round() as usize;
        let mut out_frames = Vec::with_capacity(out_n);

        // TODO Use a proper resampler, like https://github.com/HEnquist/rubato
        for i in 0..out_n {
            // Simple linear interpolation:
            let x = i as f32 / factor;
            let t = x.fract();
            let i0 = x.floor() as usize;
            let i1 = i0 + 1;
            let f0 = self.frames[i0];
            let f1 = self.frames[i1.min(self.frames.len() - 1)];
            let frame = f0 * (1.0 - t) + f1 * t;
            out_frames.push(frame);
        }

        Self {
            frames: out_frames,
            frame_rate: target_frame_rate,
        }
    }

    pub fn fade_in_out(&self) -> Self {
        re_tracing::profile_function!();

        let mut out_frames = Vec::with_capacity(FADE_FRAMES + self.frames.len() + FADE_FRAMES);

        if let Some(front) = self.frames.first().copied() {
            // Fade-in:
            for i in 0..FADE_FRAMES {
                let t = (i as f32 + 0.5) / FADE_FRAMES as f32;
                out_frames.push(t * front);
            }
        }

        out_frames.extend_from_slice(&self.frames);

        if let Some(last) = self.frames.last().copied() {
            // Fade-out:
            for i in 0..FADE_FRAMES {
                let t = 1.0 - (i as f32 + 0.5) / FADE_FRAMES as f32;
                out_frames.push(t * last);
            }
        }

        Self {
            frames: out_frames,
            frame_rate: self.frame_rate,
        }
    }
}

impl TryFrom<&AudioEntry> for StereoAudio {
    type Error = String;

    fn try_from(entry: &AudioEntry) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();
        use re_types::datatypes::TensorBuffer;

        let AudioEntry {
            data,
            frame_rate: sample_rate,
            ..
        } = entry;
        let re_types::datatypes::TensorData { shape, buffer } = &**data;

        // Ignore leading and trailing unit-dimensions:
        let mut shape = shape.iter().map(|d| d.size).collect::<Vec<_>>();
        while shape.first() == Some(&1) {
            shape.remove(0);
        }
        while shape.last() == Some(&1) {
            shape.pop();
        }

        fn from_8bit(sample: u8) -> f32 {
            (sample as f32 - 128.0) / 128.0
        }

        fn from_16bit(sample: i16) -> f32 {
            sample as f32 / 32765.0
        }

        match shape.as_slice() {
            [] => Ok(Default::default()),

            // Mono
            [_] => match buffer {
                TensorBuffer::U8(data) => Ok(StereoAudio::from_mono(
                    *sample_rate,
                    data.iter().copied().map(from_8bit),
                )),
                TensorBuffer::I16(data) => Ok(StereoAudio::from_mono(
                    *sample_rate,
                    data.iter().copied().map(from_16bit),
                )),
                TensorBuffer::F32(data) => {
                    Ok(StereoAudio::from_mono(*sample_rate, data.iter().copied()))
                }
                _ => Err(format!("Unsupported audio format: {}", buffer.dtype())),
            },

            // Stereo interleaved (L,R,L,R,…)
            [_, 2] => match buffer {
                TensorBuffer::U8(data) => Ok(StereoAudio::from_stereo(
                    *sample_rate,
                    data.iter().copied().map(from_8bit),
                )),
                TensorBuffer::I16(data) => Ok(StereoAudio::from_stereo(
                    *sample_rate,
                    data.iter().copied().map(from_16bit),
                )),
                TensorBuffer::F32(data) => {
                    Ok(StereoAudio::from_stereo(*sample_rate, data.iter().copied()))
                }
                _ => Err(format!("Unsupported audio format: {}", buffer.dtype())),
            },

            // Non-interleaved stereo (weird)
            [2, _] => Err("Audio with non-interleaved channels not yet supported".to_owned()),

            [_, channels] => Err(format!("Audio with {channels} not yet supported")),

            shape => Err(format!("Audio buffer had strange shape: {shape:?}")),
        }
    }
}

struct AudioQueue {
    /// The next frames to be played
    // TODO: this should be a map of sources, keyed on `(SpaceViewId, InstancePath)` (or a hash thereof),
    // so that we can clear out one source without affecting the others.
    // This is important when scrubbing.
    frames: VecDeque<StereoFrame>,

    /// Frame rate of the audio device, e.g. 44100 Hz
    frame_rate: f32,

    /// Total number of consumed frames
    frame_nr: u64,
}

impl AudioQueue {
    pub fn new(frame_rate: f32) -> Self {
        Self {
            frames: VecDeque::new(),
            frame_rate,
            frame_nr: 0,
        }
    }

    /// Called by the audio device periodically
    fn fill_buffer(&mut self, buffer: &mut [f32], num_channels: usize) {
        re_tracing::profile_function!();
        assert_eq!(num_channels, 2);
        for out_frame in buffer.chunks_mut(num_channels) {
            let frame = self.frames.pop_front().unwrap_or_default();
            out_frame[0] = frame.left;
            out_frame[1] = frame.right;
            self.frame_nr += 1;
        }
    }

    fn play(&mut self, audio: StereoAudio) {
        re_tracing::profile_function!();
        let audio = audio.resample(self.frame_rate);
        let audio = audio.fade_in_out();

        let StereoAudio {
            frames,
            frame_rate: _, // should be correct at this point
        } = audio;

        let append = false;

        let volume = 0.75; // TODO

        if append {
            for sample in frames {
                self.frames.push_back(volume * sample);
            }
        } else {
            // mix
            for (i, frame) in frames.iter().copied().enumerate() {
                if i < self.frames.len() {
                    self.frames[i] += volume * frame;
                } else {
                    self.frames.push_back(volume * frame);
                }
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(front) = self.frames.front().copied() {
            self.frames.clear();
            // Fade-out to avoid clicks:
            for i in 0..FADE_FRAMES {
                let t = 1.0 - (i as f32 + 0.5) / FADE_FRAMES as f32;
                self.frames.push_back(t * front);
            }
        } else {
            self.frames.clear();
        }
    }
}

pub struct AudioPlayer {
    queue: Arc<Mutex<AudioQueue>>,
    // device: Option<Box<dyn tinyaudio::BaseAudioOutputDevice>>,
}

impl AudioPlayer {
    fn new() -> Self {
        let sample_rate = 48000; // TODO: make configurable

        let params = tinyaudio::OutputDeviceParameters {
            channels_count: 2,
            sample_rate,
            channel_sample_count: 2048, // higher = more latency
        };

        let queue = Arc::new(Mutex::new(AudioQueue::new(sample_rate as _)));

        let device = tinyaudio::run_output_device(params, {
            let queue = queue.clone();
            move |data| {
                queue.lock().fill_buffer(data, params.channels_count);
            }
        });

        match device {
            Ok(device) => {
                re_log::debug!("Created audio output device");
                std::mem::forget(device);
            }
            Err(err) => {
                re_log::error!("Failed to create audio output device: {err}");
            }
        };

        Self { queue }
    }

    pub fn play(&self, audio: StereoAudio) {
        self.queue.lock().play(audio);
    }

    pub fn stop(&self) {
        self.queue.lock().stop();
    }
}
