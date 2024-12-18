use crate::{
    components::ImageBuffer,
    datatypes::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat, TensorData},
    image::{
        blob_and_datatype_from_tensor, find_non_empty_dim_indices, ImageChannelType,
        ImageConstructionError,
    },
};

#[cfg(feature = "image")]
use crate::image::{ImageConversionError, ImageLoadError};

use super::EncodedImage;

use super::Image;

impl Image {
    /// Try to construct an [`Image`] from a color model (L, RGB, RGBA, …) and anything that can be converted into [`TensorData`].
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data does not match the given color model.
    ///
    /// This is useful for constructing an [`Image`] from an ndarray.
    ///
    /// See also [`Self::from_pixel_format`].
    pub fn from_color_model_and_tensor<T>(
        color_model: ColorModel,
        data: T,
    ) -> Result<Self, ImageConstructionError<T>>
    where
        T: TryInto<TensorData>,
        <T as TryInto<TensorData>>::Error: std::error::Error,
    {
        let tensor_data: TensorData = data
            .try_into()
            .map_err(ImageConstructionError::TensorDataConversion)?;
        let TensorData { shape, buffer, .. } = tensor_data;

        let non_empty_dim_inds = find_non_empty_dim_indices(&shape);

        let is_shape_correct = match color_model {
            ColorModel::L => non_empty_dim_inds.len() == 2,
            ColorModel::RGB | ColorModel::BGR => {
                non_empty_dim_inds.len() == 3 && shape[non_empty_dim_inds[2]] == 3
            }
            ColorModel::RGBA | ColorModel::BGRA => {
                non_empty_dim_inds.len() == 3 && shape[non_empty_dim_inds[2]] == 4
            }
        };

        if !is_shape_correct {
            return Err(ImageConstructionError::BadImageShape(shape));
        }

        let (blob, datatype) = blob_and_datatype_from_tensor(buffer);

        let (height, width) = (shape[non_empty_dim_inds[0]], shape[non_empty_dim_inds[1]]);

        let image_format = ImageFormat {
            width: width as _,
            height: height as _,
            pixel_format: None,
            channel_datatype: Some(datatype),
            color_model: Some(color_model),
        };

        Ok(Self {
            buffer: blob.into(),
            format: image_format.into(),
            opacity: None,
            draw_order: None,
        })
    }

    /// Construct an image from a byte buffer given its resolution and pixel format.
    ///
    /// See also [`Self::from_color_model_and_tensor`].
    pub fn from_pixel_format(
        [width, height]: [u32; 2],
        pixel_format: PixelFormat,
        bytes: impl Into<ImageBuffer>,
    ) -> Self {
        let buffer = bytes.into();

        let image_format = ImageFormat::from_pixel_format([width, height], pixel_format);

        let num_expected_bytes = image_format.num_bytes();
        if buffer.len() != num_expected_bytes {
            re_log::warn_once!(
                "Expected {width}x{height} {pixel_format:?} image to be {num_expected_bytes} B, but got {} B", buffer.len()
            );
        }

        Self {
            buffer,
            format: image_format.into(),
            opacity: None,
            draw_order: None,
        }
    }

    /// Construct an image from a byte buffer given its resolution, color model, and data type.
    ///
    /// See also [`Self::from_color_model_and_tensor`].
    pub fn from_color_model_and_bytes(
        bytes: impl Into<ImageBuffer>,
        [width, height]: [u32; 2],
        color_model: ColorModel,
        datatype: ChannelDatatype,
    ) -> Self {
        let buffer = bytes.into();

        let image_format = ImageFormat {
            width,
            height,
            pixel_format: None,
            channel_datatype: Some(datatype),
            color_model: Some(color_model),
        };

        let num_expected_bytes = image_format.num_bytes();
        if buffer.len() != num_expected_bytes {
            re_log::warn_once!(
                "Expected {width}x{height} {color_model:?} {datatype:?} image to be {num_expected_bytes} B, but got {} B", buffer.len()
            );
        }

        Self {
            buffer,
            format: image_format.into(),
            opacity: None,
            draw_order: None,
        }
    }

    /// Construct an image from a byte buffer given its resolution, color model,
    /// and using the data type of the given vector.
    pub fn from_elements<T: ImageChannelType>(
        elements: &[T],
        [width, height]: [u32; 2],
        color_model: ColorModel,
    ) -> Self {
        let datatype = T::CHANNEL_TYPE;
        let bytes: &[u8] = bytemuck::cast_slice(elements);
        Self::from_color_model_and_bytes(
            re_types_core::ArrowBuffer::<u8>::from(bytes),
            [width, height],
            color_model,
            datatype,
        )
    }

    /// From an 8-bit grayscale image.
    pub fn from_l8(bytes: impl Into<ImageBuffer>, resolution: [u32; 2]) -> Self {
        Self::from_color_model_and_bytes(bytes, resolution, ColorModel::L, ChannelDatatype::U8)
    }

    /// Assumes RGB, 8-bit per channel, interleaved as `RGBRGBRGB`.
    pub fn from_rgb24(bytes: impl Into<ImageBuffer>, resolution: [u32; 2]) -> Self {
        Self::from_color_model_and_bytes(bytes, resolution, ColorModel::RGB, ChannelDatatype::U8)
    }

    /// Assumes RGBA, 8-bit per channel, with separate alpha.
    pub fn from_rgba32(bytes: impl Into<ImageBuffer>, resolution: [u32; 2]) -> Self {
        Self::from_color_model_and_bytes(bytes, resolution, ColorModel::RGBA, ChannelDatatype::U8)
    }

    /// Creates a new [`Image`] from a file.
    ///
    /// The image format will be inferred from the path (extension), or the contents if that fails.
    #[deprecated = "Use EncodedImage::from_file instead"]
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_file_path(filepath: impl AsRef<std::path::Path>) -> std::io::Result<EncodedImage> {
        EncodedImage::from_file(filepath)
    }

    /// Creates a new [`Image`] from the contents of a file.
    ///
    /// If unspecified, the image format will be inferred from the contents.
    #[deprecated = "Use EncodedImage::from_file_contents instead"]
    #[cfg(feature = "image")]
    #[inline]
    pub fn from_file_contents(
        contents: Vec<u8>,
        _format: Option<image::ImageFormat>,
    ) -> EncodedImage {
        EncodedImage::from_file_contents(contents)
    }
}

#[cfg(feature = "image")]
impl Image {
    /// Construct a tensor from the contents of an image file.
    ///
    /// This will spend CPU cycles decoding the image.
    /// To save CPU time and storage, we recommend you instead use
    /// [`EncodedImage::from_file_contents`].
    ///
    /// Requires the `image` feature.
    #[inline]
    pub fn from_image_bytes(
        format: image::ImageFormat,
        file_contents: &[u8],
    ) -> Result<Self, ImageLoadError> {
        re_tracing::profile_function!(format!("{format:?}"));
        let image = image::load_from_memory_with_format(file_contents, format)?;
        Ok(Self::from_image(image)?)
    }

    /// Construct a tensor from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(image: impl Into<image::DynamicImage>) -> Result<Self, ImageConversionError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(image: image::DynamicImage) -> Result<Self, ImageConversionError> {
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
                Err(ImageConversionError::UnsupportedImageColorType(
                    image.color(),
                ))
            }
        }
    }
}
