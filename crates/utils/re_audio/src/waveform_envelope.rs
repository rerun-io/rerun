use emath::{NumExt as _, Rangef};

use crate::AudioBuffer;

/// Per-bucket min/max sample values over equal slices of an [`AudioBuffer`].
#[derive(Clone, Debug, Default, PartialEq, re_byte_size::SizeBytes)]
pub struct WaveformEnvelope {
    /// Frames in the source, so the last bucket may cover fewer than `frames_per_bucket`.
    pub num_frames: usize,

    /// How many frames of the source each bucket covers. The last bucket may cover fewer.
    pub frames_per_bucket: usize,

    pub buckets: Vec<Rangef>,
}

impl AudioBuffer {
    /// Computes a min/max envelope with at most `max_buckets` buckets,
    /// for drawing waveforms without touching every sample each frame.
    ///
    /// Each frame is first downmixed to mono by averaging its channels,
    /// so the envelope is a single waveform of what a mono speaker would play.
    /// Content that is out of phase between channels cancels out,
    /// and a sound on only one of two channels shows at half amplitude.
    pub fn waveform_envelope(&self, max_buckets: usize) -> WaveformEnvelope {
        re_tracing::profile_function!();

        let num_frames = self.num_frames();
        let num_channels = self.num_channels as usize;
        if num_frames == 0 || num_channels == 0 || max_buckets == 0 {
            return WaveformEnvelope::default();
        }

        let frames_per_bucket = num_frames.div_ceil(max_buckets);
        let buckets = self
            .samples
            .chunks(frames_per_bucket * num_channels)
            .map(|bucket| {
                bucket
                    .chunks_exact(num_channels)
                    .map(|frame| frame.iter().sum::<f32>() / num_channels as f32)
                    .fold(Rangef::NOTHING, |range, mixed| {
                        range.union(Rangef::point(mixed))
                    })
            })
            .collect();

        WaveformEnvelope {
            num_frames,
            frames_per_bucket,
            buckets,
        }
    }
}

impl WaveformEnvelope {
    /// The sample range covered by `t`, given as fractions in `[0, 1]` of the total duration.
    pub fn range(&self, t: Rangef) -> Option<Rangef> {
        let n = self.buckets.len();
        if n == 0 || self.frames_per_bucket == 0 {
            return None;
        }
        // Map through frames, not bucket indices: the last bucket may be shorter than the rest.
        let first_frame = (t.min.clamp(0.0, 1.0) * self.num_frames as f32) as usize;
        let last_frame = (t.max.clamp(0.0, 1.0) * self.num_frames as f32).ceil() as usize;
        let first = (first_frame / self.frames_per_bucket).at_most(n - 1);
        let last = last_frame
            .div_ceil(self.frames_per_bucket)
            .clamp(first + 1, n);
        self.buckets[first..last]
            .iter()
            .copied()
            .reduce(Rangef::union)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChannelLayout;

    #[test]
    fn envelope_of_empty_buffer_is_empty() {
        let buffer = AudioBuffer {
            sample_rate: 48000,
            num_channels: 2,
            layout: ChannelLayout::Unknown,
            samples: vec![],
        };
        assert_eq!(buffer.num_frames(), 0);
        assert_eq!(buffer.duration_secs(), 0.0);
        assert!(buffer.waveform_envelope(16).buckets.is_empty());
    }

    #[test]
    fn envelope_mixes_channels_and_covers_all_frames() {
        let buffer = AudioBuffer {
            sample_rate: 4,
            num_channels: 2,
            layout: ChannelLayout::Unknown,
            samples: vec![1.0, -1.0, 0.5, 0.5, -0.5, -0.5, 0.0, 1.0],
        };
        assert_eq!(buffer.num_frames(), 4);
        assert_eq!(buffer.duration_secs(), 1.0);

        let envelope = buffer.waveform_envelope(2);
        assert_eq!(envelope.frames_per_bucket, 2);
        assert_eq!(
            envelope.buckets,
            vec![Rangef::new(0.0, 0.5), Rangef::new(-0.5, 0.5)]
        );
        assert_eq!(
            envelope.range(Rangef::new(0.0, 1.0)),
            Some(Rangef::new(-0.5, 0.5))
        );
        assert_eq!(
            envelope.range(Rangef::new(0.0, 0.5)),
            Some(Rangef::new(0.0, 0.5))
        );
        assert_eq!(
            envelope.range(Rangef::new(0.9, 1.0)),
            Some(Rangef::new(-0.5, 0.5))
        );
    }

    #[test]
    fn last_bucket_may_be_shorter() {
        let buffer = AudioBuffer {
            sample_rate: 1,
            num_channels: 1,
            layout: ChannelLayout::Unknown,
            samples: vec![0.1, 0.2, 0.3],
        };
        let envelope = buffer.waveform_envelope(2);
        assert_eq!(envelope.num_frames, 3);
        assert_eq!(envelope.frames_per_bucket, 2);
        assert_eq!(
            envelope.buckets,
            vec![Rangef::new(0.1, 0.2), Rangef::point(0.3)]
        );
        // 0.6 of the duration is still inside the first (two-frame) bucket.
        assert_eq!(
            envelope.range(Rangef::point(0.6)),
            Some(Rangef::new(0.1, 0.2))
        );
    }

    #[test]
    fn more_buckets_than_frames() {
        let buffer = AudioBuffer {
            sample_rate: 1,
            num_channels: 1,
            layout: ChannelLayout::Unknown,
            samples: vec![0.25, -0.75],
        };
        let envelope = buffer.waveform_envelope(4);
        assert_eq!(envelope.frames_per_bucket, 1);
        assert_eq!(
            envelope.buckets,
            vec![Rangef::point(0.25), Rangef::point(-0.75)]
        );
    }
}
