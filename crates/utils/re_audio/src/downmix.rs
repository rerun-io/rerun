//! Stereo fold-down of multichannel audio,
//! so that center and surround channels are heard instead of dropped.

use crate::{AudioBuffer, ChannelLayout, ChannelPosition};

impl AudioBuffer {
    /// Folds more than two channels down to stereo. Mono and stereo buffers are returned unchanged.
    ///
    /// Positioned channels use the ITU-R BS.775 stereo downmix weights:
    /// front left and right at full weight, other left and right speakers at -3 dB,
    /// centers at -3 dB to both sides, and the low-frequency channel dropped.
    /// Each side is normalized to preserve PCM range.
    /// Channels with an unknown layout are averaged.
    pub fn downmix_to_stereo(self) -> Self {
        let num_channels = self.num_channels as usize;
        if num_channels <= 2 {
            return self;
        }
        re_tracing::profile_function!();

        let mut lr_weights: Vec<(f32, f32)> = match &self.layout {
            ChannelLayout::Positioned(positions) if positions.len() == num_channels => {
                positions.iter().map(|p| stereo_weights(*p)).collect()
            }
            _ => {
                let weight = 1.0 / num_channels as f32;
                vec![(weight, weight); num_channels]
            }
        };
        normalize_weights(&mut lr_weights);

        let mut samples = Vec::with_capacity(self.num_frames() * 2);
        for frame in self.samples.chunks_exact(num_channels) {
            let mut left = 0.0;
            let mut right = 0.0;
            for (sample, (left_weight, right_weight)) in std::iter::zip(frame, &lr_weights) {
                left += sample * left_weight;
                right += sample * right_weight;
            }
            samples.push(left);
            samples.push(right);
        }

        Self {
            sample_rate: self.sample_rate,
            num_channels: 2,
            layout: ChannelLayout::Positioned(vec![
                ChannelPosition::FrontLeft,
                ChannelPosition::FrontRight,
            ]),
            samples,
        }
    }
}

fn normalize_weights(weights: &mut [(f32, f32)]) {
    let (left_sum, right_sum) = weights.iter().fold((0.0_f32, 0.0_f32), |sum, weight| {
        (sum.0 + weight.0.abs(), sum.1 + weight.1.abs())
    });
    let left_scale = 1.0 / left_sum.max(1.0);
    let right_scale = 1.0 / right_sum.max(1.0);
    for (left, right) in weights {
        *left *= left_scale;
        *right *= right_scale;
    }
}

/// `(left, right)` weight of one channel in a stereo fold-down.
fn stereo_weights(position: ChannelPosition) -> (f32, f32) {
    const HALF_POWER: f32 = std::f32::consts::FRAC_1_SQRT_2;
    match position {
        ChannelPosition::FrontLeft => (1.0, 0.0),
        ChannelPosition::FrontRight => (0.0, 1.0),
        ChannelPosition::Center => (HALF_POWER, HALF_POWER),
        ChannelPosition::Left => (HALF_POWER, 0.0),
        ChannelPosition::Right => (0.0, HALF_POWER),
        ChannelPosition::LowFrequency => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_5_1_to_stereo() {
        let half_power = std::f32::consts::FRAC_1_SQRT_2;
        let scale = 1.0 / (1.0 + 2.0 * half_power);
        let buffer = AudioBuffer {
            sample_rate: 1,
            num_channels: 6,
            layout: ChannelLayout::Positioned(vec![
                ChannelPosition::FrontLeft,
                ChannelPosition::FrontRight,
                ChannelPosition::Center,
                ChannelPosition::LowFrequency,
                ChannelPosition::Left,
                ChannelPosition::Right,
            ]),
            #[rustfmt::skip]
            samples: vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 100.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        };
        let stereo = buffer.downmix_to_stereo();
        assert_eq!(stereo.num_channels, 2);
        assert_eq!(
            stereo.samples,
            [
                scale,
                0.0,
                half_power * scale,
                half_power * scale,
                0.0,
                half_power * scale,
            ]
        );
    }

    #[test]
    fn positioned_downmix_preserves_pcm_range() {
        let buffer = AudioBuffer {
            sample_rate: 1,
            num_channels: 6,
            layout: ChannelLayout::Positioned(vec![
                ChannelPosition::FrontLeft,
                ChannelPosition::FrontRight,
                ChannelPosition::Center,
                ChannelPosition::LowFrequency,
                ChannelPosition::Left,
                ChannelPosition::Right,
            ]),
            samples: vec![1.0; 6],
        };

        let stereo = buffer.downmix_to_stereo();
        assert!(stereo.samples.iter().all(|sample| sample.abs() <= 1.0));
        assert!(
            stereo
                .samples
                .iter()
                .all(|sample| (sample - 1.0).abs() <= f32::EPSILON)
        );
    }

    #[test]
    fn unknown_layout_is_averaged_and_stereo_is_untouched() {
        let quad = AudioBuffer {
            sample_rate: 1,
            num_channels: 4,
            layout: ChannelLayout::Unknown,
            samples: vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 4.0],
        };
        assert_eq!(quad.downmix_to_stereo().samples, [1.0, 1.0, 1.0, 1.0]);

        let stereo = AudioBuffer {
            sample_rate: 1,
            num_channels: 2,
            layout: ChannelLayout::Unknown,
            samples: vec![0.5, -0.5],
        };
        assert_eq!(stereo.clone().downmix_to_stereo(), stereo);
    }
}
