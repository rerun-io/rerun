use super::{ChannelDatatype, ColorModel, ImageFormat};

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

    /// Determine if the image format has an alpha channel.
    #[inline]
    pub fn has_alpha(&self) -> bool {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.has_alpha()
        } else {
            self.color_model
                .as_ref()
                .map_or(false, |color_model| color_model.has_alpha())
        }
    }

    /// Determine if the image format represents floating point data.
    #[inline]
    pub fn is_float(&self) -> bool {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.is_float()
        } else {
            self.channel_datatype
                .as_ref()
                .map_or(false, |datatype| datatype.is_float())
        }
    }

    /// Number of bits needed to represent a single pixel.
    ///
    /// Note that this is not necessarily divisible by 8!
    #[inline]
    pub fn bits_per_pixel(&self) -> usize {
        if let Some(pixel_format) = self.pixel_format {
            pixel_format.bits_per_pixel()
        } else {
            self.color_model.unwrap_or_default().num_channels()
                * self.channel_datatype.unwrap_or_default().bits()
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
