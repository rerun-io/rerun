use super::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat};

impl ImageFormat {
    /// Create a new depth image format with the given resolution and datatype.
    #[inline]
    pub fn depth([width, height]: [u32; 2], datatype: ChannelDatatype) -> Self {
        Self {
            width,
            height,
            pixel_format: None,
            channel_datatype: Some(datatype),
            color_model: None,
        }
    }

    /// Create a new segmentation image format with the given resolution and datatype.
    #[inline]
    pub fn segmentation([width, height]: [u32; 2], datatype: ChannelDatatype) -> Self {
        Self {
            width,
            height,
            pixel_format: None,
            channel_datatype: Some(datatype),
            color_model: None,
        }
    }

    /// Create a new grayscale image format with 8 bit for the single channel with the given
    /// resolution.
    #[inline]
    pub fn l8([width, height]: [u32; 2]) -> Self {
        Self {
            width,
            height,
            pixel_format: None,
            channel_datatype: Some(ChannelDatatype::U8),
            color_model: Some(ColorModel::L),
        }
    }

    /// Create a new rgb image format with 8 bit per channel with the given resolution.
    #[inline]
    pub fn rgb8([width, height]: [u32; 2]) -> Self {
        Self {
            width,
            height,
            pixel_format: None,
            channel_datatype: Some(ChannelDatatype::U8),
            color_model: Some(ColorModel::RGB),
        }
    }

    /// Create a new rgba image format with 8 bit per channel with the given resolution.
    #[inline]
    pub fn rgba8([width, height]: [u32; 2]) -> Self {
        Self {
            width,
            height,
            pixel_format: None,
            channel_datatype: Some(ChannelDatatype::U8),
            color_model: Some(ColorModel::RGBA),
        }
    }

    /// From a speicifc pixel format.
    #[inline]
    pub fn from_pixel_format([width, height]: [u32; 2], pixel_format: PixelFormat) -> Self {
        Self {
            width,
            height,
            pixel_format: Some(pixel_format),
            channel_datatype: None,
            color_model: None,
        }
    }

    /// Create a new image format from a color model and datatype.
    #[inline]
    pub fn from_color_model(
        [width, height]: [u32; 2],
        color_model: ColorModel,
        datatype: ChannelDatatype,
    ) -> Self {
        Self {
            width,
            height,
            pixel_format: None,
            channel_datatype: Some(datatype),
            color_model: Some(color_model),
        }
    }

    /// Determine if the image format has an alpha channel.
    #[inline]
    pub fn has_alpha(&self) -> bool {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.has_alpha()
        } else {
            self.color_model.unwrap_or_default().has_alpha()
        }
    }

    /// Determine if the image format represents floating point data.
    #[inline]
    pub fn is_float(&self) -> bool {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.is_float()
        } else {
            self.channel_datatype.unwrap_or_default().is_float()
        }
    }

    /// Number of bytes for the whole image.
    #[inline]
    pub fn num_bytes(&self) -> usize {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.num_bytes([self.width, self.height])
        } else {
            let bits_per_pixel = self.color_model.unwrap_or_default().num_channels()
                * self.channel_datatype.unwrap_or_default().bits();
            // rounding upwards:
            (self.width as usize * self.height as usize * bits_per_pixel).div_ceil(8)
        }
    }

    /// The color model represented by this image format.
    #[inline]
    pub fn color_model(&self) -> ColorModel {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.color_model()
        } else {
            self.color_model.unwrap_or_default()
        }
    }

    /// The datatype represented by this image format.
    #[inline]
    pub fn datatype(&self) -> ChannelDatatype {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.datatype()
        } else {
            self.channel_datatype.unwrap_or_default()
        }
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(pixel_format) = self.pixel_format {
            write!(f, "{} {}×{}", pixel_format, self.width, self.height)
        } else {
            write!(
                f,
                "{} {} {}×{}",
                self.color_model(),
                self.datatype(),
                self.width,
                self.height
            )
        }
    }
}
