use super::ImageFormat;
use crate::datatypes::{self, ChannelDatatype, ColorModel, PixelFormat};

impl ImageFormat {
    /// Create a new depth image format with the given resolution and datatype.
    #[inline]
    pub fn depth([width, height]: [u32; 2], datatype: ChannelDatatype) -> Self {
        datatypes::ImageFormat::depth([width, height], datatype).into()
    }

    /// Create a new segmentation image format with the given resolution and datatype.
    #[inline]
    pub fn segmentation([width, height]: [u32; 2], datatype: ChannelDatatype) -> Self {
        datatypes::ImageFormat::segmentation([width, height], datatype).into()
    }

    /// Create a new rgb image format with 8 bit per channel with the given resolution.
    #[inline]
    pub fn rgb8([width, height]: [u32; 2]) -> Self {
        datatypes::ImageFormat::rgb8([width, height]).into()
    }

    /// Create a new rgba image format with 8 bit per channel with the given resolution.
    #[inline]
    pub fn rgba8([width, height]: [u32; 2]) -> Self {
        datatypes::ImageFormat::rgba8([width, height]).into()
    }

    /// From a specific pixel format.
    #[inline]
    pub fn from_pixel_format([width, height]: [u32; 2], pixel_format: PixelFormat) -> Self {
        datatypes::ImageFormat::from_pixel_format([width, height], pixel_format).into()
    }

    /// Determine if the image format has an alpha channel.
    #[inline]
    pub fn has_alpha(&self) -> bool {
        self.0.has_alpha()
    }

    /// Determine if the image format represents floating point data.
    #[inline]
    pub fn is_float(&self) -> bool {
        self.0.is_float()
    }

    /// Number of bytes for the whole image.
    #[inline]
    pub fn num_bytes(&self) -> usize {
        self.0.num_bytes()
    }

    /// The color model represented by this image format.
    #[inline]
    pub fn color_model(&self) -> ColorModel {
        self.0.color_model()
    }

    /// The datatype represented by this image format.
    #[inline]
    pub fn datatype(&self) -> ChannelDatatype {
        self.0.datatype()
    }
}
