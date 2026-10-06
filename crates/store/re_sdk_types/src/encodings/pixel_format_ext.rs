use super::{ChannelDatatype, ColorModel, PixelFormat};
use crate::{
    bayer_pattern::BayerPattern,
    image::{YuvMatrixCoefficients, rgb_from_yuv},
};

impl PixelFormat {
    /// Do we have an alpha channel?
    #[inline]
    pub fn has_alpha(&self) -> bool {
        match self {
            Self::Y_U_V12_FullRange
            | Self::Y_U_V16_FullRange
            | Self::Y_U_V24_FullRange
            | Self::Y8_FullRange
            | Self::Y_U_V12_LimitedRange
            | Self::Y_U_V16_LimitedRange
            | Self::Y_U_V24_LimitedRange
            | Self::Y8_LimitedRange
            | Self::NV12
            | Self::YUY2
            | Self::BayerRGGB8
            | Self::BayerBGGR8
            | Self::BayerGBRG8
            | Self::BayerGRBG8 => false,
        }
    }

    #[inline]
    /// Is this pixel format floating point?
    pub fn is_float(&self) -> bool {
        match self {
            Self::Y_U_V12_FullRange
            | Self::Y_U_V16_FullRange
            | Self::Y_U_V24_FullRange
            | Self::Y8_FullRange
            | Self::Y_U_V12_LimitedRange
            | Self::Y_U_V16_LimitedRange
            | Self::Y_U_V24_LimitedRange
            | Self::Y8_LimitedRange
            | Self::NV12
            | Self::YUY2
            | Self::BayerRGGB8
            | Self::BayerBGGR8
            | Self::BayerGBRG8
            | Self::BayerGRBG8 => false,
        }
    }

    /// Number of bytes needed to represent an image of the given size.
    #[inline]
    pub fn num_bytes(&self, [w, h]: [u32; 2]) -> usize {
        let num_pixels = w as usize * h as usize;
        #[expect(clippy::match_same_arms)]
        match self {
            // 444 formats.
            Self::Y_U_V24_FullRange | Self::Y_U_V24_LimitedRange => num_pixels * 4,

            // 422 formats.
            Self::Y_U_V16_FullRange | Self::Y_U_V16_LimitedRange | Self::YUY2 => {
                16 * num_pixels / 8
            }

            // 420 formats.
            Self::Y_U_V12_FullRange | Self::Y_U_V12_LimitedRange | Self::NV12 => {
                12 * num_pixels / 8
            }

            // Monochrome formats.
            Self::Y8_LimitedRange | Self::Y8_FullRange => num_pixels,

            // Bayer formats.
            Self::BayerRGGB8 | Self::BayerBGGR8 | Self::BayerGBRG8 | Self::BayerGRBG8 => num_pixels,
        }
    }

    /// The color model derived from this pixel format.
    #[inline]
    pub fn color_model(&self) -> ColorModel {
        #[expect(clippy::match_same_arms)]
        match self {
            Self::Y_U_V12_FullRange
            | Self::Y_U_V16_FullRange
            | Self::Y_U_V24_FullRange
            | Self::Y_U_V12_LimitedRange
            | Self::Y_U_V16_LimitedRange
            | Self::Y_U_V24_LimitedRange
            | Self::NV12
            | Self::YUY2 => ColorModel::RGB,

            // TODO(andreas): This shouldn't be ColorModel::RGB, but our YUV converter can't do anything else right now:
            // The converter doesn't *have* to always output RGB, but having it sometimes output R(8) specifically for the
            // YUV converter requires me to do more bookkeeping (needs a new renderpipeline and I expect other ripples).
            //
            // As of writing, having this color_model "incorrectly" be RGB mostly affects hovering logic which will continue to show RGB rather than L.
            //
            // Note that this does not affect the memory Y8 needs. It just implies that we use more GPU memory than we should.
            // However, we typically (see image cache) hold the converted GPU textures only as long as we actually draw with them.
            Self::Y8_LimitedRange | Self::Y8_FullRange => ColorModel::RGB,

            Self::BayerRGGB8 | Self::BayerBGGR8 | Self::BayerGBRG8 | Self::BayerGRBG8 => {
                ColorModel::RGB
            }
        }
    }

    #[inline]
    /// The datatype that this decodes into.
    pub fn datatype(&self) -> ChannelDatatype {
        match self {
            Self::Y_U_V12_FullRange
            | Self::Y_U_V16_FullRange
            | Self::Y_U_V24_FullRange
            | Self::Y8_FullRange
            | Self::Y_U_V12_LimitedRange
            | Self::Y_U_V16_LimitedRange
            | Self::Y_U_V24_LimitedRange
            | Self::Y8_LimitedRange
            | Self::NV12
            | Self::YUY2
            | Self::BayerRGGB8
            | Self::BayerBGGR8
            | Self::BayerGBRG8
            | Self::BayerGRBG8 => ChannelDatatype::U8,
        }
    }

    /// The color filter pattern, if this is a raw Bayer format.
    #[inline]
    pub fn bayer_pattern(&self) -> Option<BayerPattern> {
        match self {
            Self::BayerRGGB8 => Some(BayerPattern::Rggb),
            Self::BayerBGGR8 => Some(BayerPattern::Bggr),
            Self::BayerGBRG8 => Some(BayerPattern::Gbrg),
            Self::BayerGRBG8 => Some(BayerPattern::Grbg),

            Self::Y_U_V12_FullRange
            | Self::Y_U_V16_FullRange
            | Self::Y_U_V24_FullRange
            | Self::Y8_FullRange
            | Self::Y_U_V12_LimitedRange
            | Self::Y_U_V16_LimitedRange
            | Self::Y_U_V24_LimitedRange
            | Self::Y8_LimitedRange
            | Self::NV12
            | Self::YUY2 => None,
        }
    }

    /// Random-access decoding of a specific pixel of a YUV image.
    ///
    /// Return `None` if out-of-range, or if this is a Bayer format.
    #[inline]
    pub fn decode_yuv_at(&self, buf: &[u8], [w, h]: [u32; 2], [x, y]: [u32; 2]) -> Option<[u8; 3]> {
        match self {
            Self::Y_U_V24_FullRange | Self::Y_U_V24_LimitedRange => {
                let plane_size = (w * h) as usize;
                let plane_coord = (y * w + x) as usize;

                let luma = *buf.get(plane_coord)?;
                let u = *buf.get(plane_coord + plane_size)?;
                let v = *buf.get(plane_coord + plane_size * 2)?;
                Some([luma, u, v])
            }

            Self::Y_U_V16_FullRange | Self::Y_U_V16_LimitedRange => {
                let y_plane_size = (w * h) as usize;
                let uv_plane_size = y_plane_size / 2; // Half horizontal resolution.
                let y_plane_coord = (y * w + x) as usize;
                let uv_plane_coord = y_plane_coord / 2; // == (y * (w / 2) + x / 2)

                let luma = *buf.get(y_plane_coord)?;
                let u = *buf.get(uv_plane_coord + y_plane_size)?;
                let v = *buf.get(uv_plane_coord + y_plane_size + uv_plane_size)?;
                Some([luma, u, v])
            }

            Self::Y_U_V12_FullRange | Self::Y_U_V12_LimitedRange => {
                let y_plane_size = (w * h) as usize;
                let uv_plane_size = y_plane_size / 4; // Half horizontal & vertical resolution.
                let y_plane_coord = (y * w + x) as usize;
                let uv_plane_coord = (y * w / 4 + x / 2) as usize; // == ((y / 2) * (w / 2) + x / 2)

                let luma = *buf.get(y_plane_coord)?;
                let u = *buf.get(uv_plane_coord + y_plane_size)?;
                let v = *buf.get(uv_plane_coord + y_plane_size + uv_plane_size)?;
                Some([luma, u, v])
            }

            Self::Y8_FullRange | Self::Y8_LimitedRange => {
                let luma = *buf.get((y * w + x) as usize)?;
                Some([luma, 128, 128])
            }

            Self::NV12 => {
                let uv_offset = w * h;
                let luma = *buf.get((y * w + x) as usize)?;
                let u = *buf.get((uv_offset + (y / 2) * w + x) as usize)?;
                let v = *buf.get((uv_offset + (y / 2) * w + x) as usize + 1)?;
                Some([luma, u, v])
            }

            Self::YUY2 => {
                let index = ((y * w + x) * 2) as usize;
                if x % 2 == 0 {
                    Some([*buf.get(index)?, *buf.get(index + 1)?, *buf.get(index + 3)?])
                } else {
                    Some([*buf.get(index)?, *buf.get(index - 1)?, *buf.get(index + 1)?])
                }
            }

            Self::BayerRGGB8 | Self::BayerBGGR8 | Self::BayerGBRG8 | Self::BayerGRBG8 => None,
        }
    }

    /// Returns true if the format is a YUV format using
    /// limited range YUV.
    ///
    /// I.e. for 8bit data, Y is valid in [16, 235] and U/V [16, 240], rather than 0-255.
    pub fn is_limited_yuv_range(&self) -> bool {
        match self {
            Self::Y_U_V24_LimitedRange
            | Self::Y_U_V16_LimitedRange
            | Self::Y_U_V12_LimitedRange
            | Self::Y8_LimitedRange
            | Self::NV12
            | Self::YUY2 => true,

            Self::Y_U_V24_FullRange
            | Self::Y_U_V12_FullRange
            | Self::Y_U_V16_FullRange
            | Self::Y8_FullRange
            | Self::BayerRGGB8
            | Self::BayerBGGR8
            | Self::BayerGBRG8
            | Self::BayerGRBG8 => false,
        }
    }

    /// Yuv matrix coefficients used by this format.
    ///
    /// Bayer formats aren't YUV, they return [`YuvMatrixCoefficients::Bt709`].
    // TODO(andreas): Expose this in the API separately and document it better.
    pub fn yuv_matrix_coefficients(&self) -> YuvMatrixCoefficients {
        match self {
            Self::Y_U_V24_LimitedRange
            | Self::Y_U_V24_FullRange
            | Self::Y_U_V12_LimitedRange
            | Self::Y_U_V12_FullRange
            | Self::Y_U_V16_LimitedRange
            | Self::Y_U_V16_FullRange
            // TODO(andreas): Y8 isn't really color, does this even make sense?
            | Self::Y8_FullRange
            | Self::Y8_LimitedRange
            | Self::BayerRGGB8
            | Self::BayerBGGR8
            | Self::BayerGBRG8
            | Self::BayerGRBG8 => YuvMatrixCoefficients::Bt709,

            Self::NV12 | Self::YUY2 => YuvMatrixCoefficients::Bt601,
        }
    }

    /// Random-access decoding of a specific pixel of an image.
    ///
    /// Bayer formats are demosaiced with [`BayerPattern::demosaic_at`].
    ///
    /// Return `None` if out-of-range.
    #[inline]
    pub fn decode_rgb_at(&self, buf: &[u8], [w, h]: [u32; 2], [x, y]: [u32; 2]) -> Option<[u8; 3]> {
        if let Some(pattern) = self.bayer_pattern() {
            return pattern.demosaic_at([w, h], [x, y], |[x, y]| {
                buf.get(y as usize * w as usize + x as usize).copied()
            });
        }

        let [y, u, v] = self.decode_yuv_at(buf, [w, h], [x, y])?;
        Some(rgb_from_yuv(
            y,
            u,
            v,
            self.is_limited_yuv_range(),
            self.yuv_matrix_coefficients(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAYER_FORMATS: [PixelFormat; 4] = [
        PixelFormat::BayerRGGB8,
        PixelFormat::BayerBGGR8,
        PixelFormat::BayerGBRG8,
        PixelFormat::BayerGRBG8,
    ];

    /// Mosaics an RGB image with the pattern of the given format into the raw buffer of that format.
    fn mosaic(format: PixelFormat, [w, h]: [u32; 2], rgb: impl Fn([u32; 2]) -> [u8; 3]) -> Vec<u8> {
        let pattern = format.bayer_pattern().unwrap();
        let mut buf = Vec::new();
        for y in 0..h {
            for x in 0..w {
                buf.push(rgb([x, y])[pattern.channel_at([x, y])]);
            }
        }
        assert_eq!(buf.len(), format.num_bytes([w, h]));
        buf
    }

    /// A uniformly colored image comes out with the same color at every pixel, including the borders.
    #[test]
    fn bayer_uniform_color() {
        let size = [5, 4];
        let color = [40, 130, 250];
        for format in BAYER_FORMATS {
            let buf = mosaic(format, size, |_| color);

            for y in 0..size[1] {
                for x in 0..size[0] {
                    assert_eq!(
                        format.decode_rgb_at(&buf, size, [x, y]),
                        Some(color),
                        "{format} at {x}, {y}"
                    );
                }
            }
            assert_eq!(format.decode_rgb_at(&buf, size, [size[0], 0]), None);
        }
    }

    /// A horizontal gradient is reconstructed exactly away from the borders, since bilinear
    /// interpolation reproduces linear ramps.
    #[test]
    fn bayer_linear_gradient() {
        let size = [8, 6];
        let gradient = |[x, _y]: [u32; 2]| [10 * x as u8, 20 * x as u8, 30 * x as u8];

        for format in BAYER_FORMATS {
            let buf = mosaic(format, size, gradient);

            for y in 1..size[1] - 1 {
                for x in 1..size[0] - 1 {
                    assert_eq!(
                        format.decode_rgb_at(&buf, size, [x, y]),
                        Some(gradient([x, y])),
                        "{format} at {x}, {y}"
                    );
                }
            }
        }
    }
}
