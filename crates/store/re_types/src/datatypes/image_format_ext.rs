use super::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat};

impl ImageFormat {
    #[inline]
    /// Create a new depth image format with the given resolution and datatype.
    pub fn depth(width: u32, height: u32, datatype: ChannelDatatype) -> Self {
        Self {
            width,
            height,
            pixel_format: PixelFormat::GENERIC,
            channel_datatype: Some(datatype),
            color_model: None,
        }
    }

    #[inline]
    /// Create a new segmentation image format with the given resolution and datatype.
    pub fn segmentation(width: u32, height: u32, datatype: ChannelDatatype) -> Self {
        Self {
            width,
            height,
            pixel_format: PixelFormat::GENERIC,
            channel_datatype: Some(datatype),
            color_model: None,
        }
    }

    #[inline]
    /// Determine if the image format has an alpha channel.
    pub fn has_alpha(&self) -> bool {
        if self.pixel_format == PixelFormat::GENERIC {
            self.color_model
                .as_ref()
                .map_or(false, |color_model| color_model.has_alpha())
        } else {
            self.pixel_format.has_alpha().unwrap_or(false)
        }
    }

    #[inline]
    /// Determine if the image format represents floating point data.
    pub fn is_float(&self) -> bool {
        if self.pixel_format == PixelFormat::GENERIC {
            self.channel_datatype
                .as_ref()
                .map_or(false, |datatype| datatype.is_float())
        } else {
            self.pixel_format.is_float().unwrap_or(false)
        }
    }

    /// Number of bits needed to represent a single pixel.
    ///
    /// Note that this is not necessarily divisible by 8!
    #[inline]
    pub fn bits_per_pixel(&self) -> usize {
        if self.pixel_format == PixelFormat::GENERIC {
            self.color_model.unwrap_or_default().num_channels()
                * self.channel_datatype.unwrap_or_default().bits()
        } else {
            // TODO(jleibs): restructure to get rid of optionals here
            self.pixel_format.bits_per_pixel().unwrap_or(0)
        }
    }

    #[inline]
    /// The color model represented by this image format.
    pub fn color_model(&self) -> ColorModel {
        if self.pixel_format == PixelFormat::GENERIC {
            self.color_model.unwrap_or_default()
        } else {
            // TODO(jleibs): restructure to get rid of optionals here
            self.pixel_format.color_model().unwrap_or_default()
        }
    }

    #[inline]
    /// The datatype represented by this image format.
    pub fn datatype(&self) -> ChannelDatatype {
        if self.pixel_format == PixelFormat::GENERIC {
            self.channel_datatype.unwrap_or_default()
        } else {
            // TODO(jleibs): What is the meaning of datatype for non-generic formats?
            ChannelDatatype::default()
        }
    }
}
