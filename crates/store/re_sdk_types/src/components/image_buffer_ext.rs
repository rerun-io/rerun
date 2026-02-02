#[cfg(feature = "image")]
use super::{ImageBuffer, ImageFormat};
#[cfg(feature = "image")]
use crate::{datatypes::ColorModel, image::ImageChannelType};

#[cfg(feature = "image")]
impl ImageBuffer {
    /// Utility method for constructing an image & format
    /// from a byte buffer given its resolution and using the data type of the given vector.
    fn from_elements<T: ImageChannelType>(
        elements: &[T],
        [width, height]: [u32; 2],
        color_model: ColorModel,
    ) -> (Self, ImageFormat) {
        let datatype = T::CHANNEL_TYPE;
        let bytes: &[u8] = bytemuck::cast_slice(elements);
        let image_format = ImageFormat::from_color_model([width, height], color_model, datatype);

        let num_expected_bytes = image_format.num_bytes();
        if bytes.len() != num_expected_bytes {
            re_log::warn_once!(
                "Expected {width}x{height} {color_model:?} {datatype} image to be {num_expected_bytes} B, but got {} B",
                bytes.len()
            );
        }

        (Self(bytes.into()), image_format)
    }

    /// Construct an image buffer & image format from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(
        image: impl Into<image::DynamicImage>,
    ) -> Result<(Self, ImageFormat), crate::image::ImageConversionError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct an image buffer & image format from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(
        image: image::DynamicImage,
    ) -> Result<(Self, ImageFormat), crate::image::ImageConversionError> {
        re_tracing::profile_function!();

        let res = [image.width(), image.height()];

        match image {
            image::DynamicImage::ImageLuma8(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::L))
            }
            image::DynamicImage::ImageLuma16(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::L))
            }

            image::DynamicImage::ImageLumaA8(image) => {
                re_log::warn!(
                    "Rerun doesn't have native support for 8-bit Luma + Alpha. The image will be convert to RGBA."
                );
                Self::from_image(image::DynamicImage::ImageLumaA8(image).to_rgba8())
            }
            image::DynamicImage::ImageLumaA16(image) => {
                re_log::warn!(
                    "Rerun doesn't have native support for 16-bit Luma + Alpha. The image will be convert to RGBA."
                );
                Self::from_image(image::DynamicImage::ImageLumaA16(image).to_rgba16())
            }

            image::DynamicImage::ImageRgb8(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::RGB))
            }
            image::DynamicImage::ImageRgb16(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::RGB))
            }
            image::DynamicImage::ImageRgb32F(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::RGB))
            }

            image::DynamicImage::ImageRgba8(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::RGBA))
            }
            image::DynamicImage::ImageRgba16(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::RGBA))
            }
            image::DynamicImage::ImageRgba32F(image) => {
                Ok(Self::from_elements(image.as_raw(), res, ColorModel::RGBA))
            }

            _ => {
                // It is very annoying that DynamicImage is #[non_exhaustive]
                Err(crate::image::ImageConversionError::UnsupportedImageColorType(image.color()))
            }
        }
    }
}
