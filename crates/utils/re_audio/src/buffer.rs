/// Decoded PCM audio, with samples interleaved by channel (`LRLR…` for stereo).
///
/// A *frame* is one sample per channel at one instant, so a stereo buffer has two samples
/// per frame and `sample_rate` frames per second.
#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct AudioBuffer {
    /// Frames per second, e.g. 48000.
    pub sample_rate: u32,

    /// Samples per frame, e.g. 2 for stereo.
    pub num_channels: u32,

    /// What each channel is, if known. Only matters for more than two channels.
    pub layout: ChannelLayout,

    /// `num_frames * num_channels` samples, nominally in `[-1, 1]`.
    ///
    /// Integer sources are scaled into that range, but float sources and lossy codecs
    /// (AAC, MP3, Vorbis) can overshoot it, so clamp before sending samples to an output device.
    pub samples: Vec<f32>,
}

/// Where one channel of an [`AudioBuffer`] is meant to be heard,
/// coarse enough for a stereo fold-down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, re_byte_size::SizeBytes)]
pub enum ChannelPosition {
    /// The main left speaker.
    FrontLeft,

    /// The main right speaker.
    FrontRight,

    /// A center speaker: front, rear, top, or bottom.
    Center,

    /// Any other speaker on the left: side, rear, top, or bottom.
    Left,

    /// Any other speaker on the right: side, rear, top, or bottom.
    Right,

    /// A subwoofer.
    LowFrequency,
}

/// What the channels of an [`AudioBuffer`] are, in interleaved order.
#[derive(Clone, Debug, Default, PartialEq, Eq, re_byte_size::SizeBytes)]
pub enum ChannelLayout {
    /// One entry per channel.
    Positioned(Vec<ChannelPosition>),

    /// Discrete, ambisonic, or unlabeled channels.
    #[default]
    Unknown,
}

impl AudioBuffer {
    /// The number of samples per channel.
    #[inline]
    pub fn num_frames(&self) -> usize {
        if self.num_channels == 0 {
            0
        } else {
            self.samples.len() / self.num_channels as usize
        }
    }

    /// Length of the audio in seconds.
    #[inline]
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.num_frames() as f64 / self.sample_rate as f64
        }
    }

    /// The sample for `channel` in the given frame, or `None` if out of range.
    #[inline]
    pub fn sample(&self, frame: usize, channel: u32) -> Option<f32> {
        if self.num_channels <= channel {
            return None;
        }
        let index = frame
            .checked_mul(self.num_channels as usize)?
            .checked_add(channel as usize)?;
        self.samples.get(index).copied()
    }

    /// The sample for `channel` at a fractional frame position,
    /// linearly interpolated between the two nearest frames.
    ///
    /// `None` outside `0.0..num_frames`.
    ///
    /// Linear interpolation is a crude way to resample audio: it dulls high frequencies and
    /// aliases when the playback rate differs much from the source rate. A proper resampler
    /// uses a windowed-sinc (polyphase) filter, see <https://ccrma.stanford.edu/~jos/resample/>.
    pub fn sample_interpolated(&self, frame: f64, channel: u32) -> Option<f32> {
        if !(0.0..self.num_frames() as f64).contains(&frame) {
            return None;
        }
        let i0 = frame as usize;
        let t = (frame - i0 as f64) as f32;
        let a = self.sample(i0, channel)?;
        let b = self.sample(i0 + 1, channel).unwrap_or(a);
        Some(emath::lerp(a..=b, t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rejects_out_of_range_and_overflowing_indices() {
        let buffer = AudioBuffer {
            sample_rate: 1,
            num_channels: 2,
            layout: ChannelLayout::Unknown,
            samples: vec![0.1, 0.2, 0.3, 0.4],
        };

        assert_eq!(buffer.sample(0, 0), Some(0.1));
        assert_eq!(buffer.sample(1, 1), Some(0.4));
        assert_eq!(buffer.sample(0, 2), None);
        assert_eq!(buffer.sample(2, 0), None);
        assert_eq!(buffer.sample(usize::MAX, 0), None);
    }

    #[test]
    fn interpolated_samples() {
        let buffer = AudioBuffer {
            sample_rate: 1,
            num_channels: 1,
            layout: ChannelLayout::Unknown,
            samples: vec![0.0, 1.0],
        };
        assert_eq!(buffer.sample_interpolated(-0.1, 0), None);
        assert_eq!(buffer.sample_interpolated(0.0, 0), Some(0.0));
        assert_eq!(buffer.sample_interpolated(0.25, 0), Some(0.25));
        assert_eq!(buffer.sample_interpolated(1.0, 0), Some(1.0));
        assert_eq!(buffer.sample_interpolated(1.5, 0), Some(1.0));
        assert_eq!(buffer.sample_interpolated(2.0, 0), None);
    }
}
