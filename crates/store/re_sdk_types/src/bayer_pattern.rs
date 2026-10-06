//! Helpers for raw Bayer images.

/// The arrangement of the color filter on a raw Bayer image.
///
/// The name lists the colors of the top-left 2x2 block, row by row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BayerPattern {
    /// Red and green in even rows, green and blue in odd rows.
    Rggb,

    /// Blue and green in even rows, green and red in odd rows.
    Bggr,

    /// Green and blue in even rows, red and green in odd rows.
    Gbrg,

    /// Green and red in even rows, blue and green in odd rows.
    Grbg,
}

impl BayerPattern {
    /// The color channel sampled at the given pixel, where 0 is red, 1 is green and 2 is blue.
    #[inline]
    pub fn channel_at(self, [x, y]: [u32; 2]) -> usize {
        let [even_row, odd_row] = match self {
            Self::Rggb => [[0, 1], [1, 2]],
            Self::Bggr => [[2, 1], [1, 0]],
            Self::Gbrg => [[1, 2], [0, 1]],
            Self::Grbg => [[1, 0], [2, 1]],
        };
        let row = if y.is_multiple_of(2) {
            even_row
        } else {
            odd_row
        };
        row[(x % 2) as usize]
    }

    /// Bilinear demosaicing of a single pixel.
    ///
    /// The channel the pixel samples is taken as is.
    /// Each other channel is the rounded average of all samples of that channel in the 3x3 neighborhood
    /// of the pixel, ignoring neighbors outside the image.
    /// `sample` returns the raw value at an in-bounds coordinate.
    ///
    /// Returns `None` if the pixel is out of bounds or `sample` returns `None`.
    pub fn demosaic_at(
        self,
        [w, h]: [u32; 2],
        [x, y]: [u32; 2],
        sample: impl Fn([u32; 2]) -> Option<u8>,
    ) -> Option<[u8; 3]> {
        if w <= x || h <= y {
            return None;
        }

        let own_channel = self.channel_at([x, y]);
        let mut sums = [0_u32; 3];
        let mut counts = [0_u32; 3];

        for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
            for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                let channel = self.channel_at([nx, ny]);
                if channel != own_channel {
                    sums[channel] += sample([nx, ny])? as u32;
                    counts[channel] += 1;
                }
            }
        }

        let own_value = sample([x, y])?;

        Some(std::array::from_fn(|channel| {
            if channel == own_channel {
                own_value
            } else {
                let count = counts[channel];
                (sums[channel] + count / 2).checked_div(count).unwrap_or(0) as u8
            }
        }))
    }
}
