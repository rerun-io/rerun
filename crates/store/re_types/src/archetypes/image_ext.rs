use crate::{
    components::{Blob, ChannelDataType, ColorModel, PixelFormat, Resolution2D},
    datatypes::TensorData,
    image::{
        blob_and_data_type_from_tensor, find_non_empty_dim_indices, ImageChannelType,
        ImageConstructionError,
    },
    tensor_data::TensorImageLoadError,
};

use super::ImageEncoded;

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
        let shape = tensor_data.shape;

        let non_empty_dim_inds = find_non_empty_dim_indices(&shape);

        let is_shape_correct = match color_model {
            ColorModel::L => non_empty_dim_inds.len() == 2,
            ColorModel::RGB => {
                non_empty_dim_inds.len() == 3 && shape[non_empty_dim_inds[2]].size == 3
            }
            ColorModel::RGBA => {
                non_empty_dim_inds.len() == 3 && shape[non_empty_dim_inds[2]].size == 4
            }
        };

        if !is_shape_correct {
            return Err(ImageConstructionError::BadImageShape(shape));
        }

        let (blob, data_type) = blob_and_data_type_from_tensor(tensor_data.buffer);

        let (height, width) = (&shape[non_empty_dim_inds[0]], &shape[non_empty_dim_inds[1]]);
        let height = height.size as u32;
        let width = width.size as u32;
        let resolution = Resolution2D::from([width, height]);

        Ok(Self {
            data: blob.into(),
            resolution,
            pixel_format: None,
            color_model: Some(color_model),
            data_type: Some(data_type),
            opacity: None,
            draw_order: None,
        })
    }

    /// Construct an image from a byte buffer given its resolution and pixel format.
    ///
    /// See also [`Self::from_color_model_and_tensor`].
    pub fn from_pixel_format(
        resolution: Resolution2D,
        pixel_format: PixelFormat,
        bytes: impl Into<Blob>,
    ) -> Self {
        let data = bytes.into();

        let actual_bytes = data.len();
        let num_expected_bytes = (resolution.area() * pixel_format.bits_per_pixel() + 7) / 8; // rounding upwards
        if data.len() != num_expected_bytes {
            re_log::warn_once!(
                "Expected {resolution} {pixel_format:?} image to be {num_expected_bytes} B, but got {actual_bytes} B",
            );
        }

        Self {
            data,
            resolution,
            pixel_format: Some(pixel_format),
            color_model: None,
            data_type: None,
            opacity: None,
            draw_order: None,
        }
    }

    /// Construct an image from a byte buffer given its resolution, color model, and data type.
    ///
    /// See also [`Self::from_color_model_and_tensor`].
    pub fn from_color_model_and_bytes(
        resolution: Resolution2D,
        color_model: ColorModel,
        data_type: ChannelDataType,
        bytes: impl Into<Blob>,
    ) -> Self {
        let data = bytes.into();

        let actual_bytes = data.len();
        let num_expected_bytes =
            (resolution.area() * color_model.num_channels() * data_type.bits() + 7) / 8; // rounding upwards
        if data.len() != num_expected_bytes {
            re_log::warn_once!(
                "Expected {resolution} {color_model:?} {data_type:?} image to be {num_expected_bytes} B, but got {actual_bytes} B",
            );
        }

        Self {
            data,
            resolution,
            pixel_format: None,
            color_model: Some(color_model),
            data_type: Some(data_type),
            opacity: None,
            draw_order: None,
        }
    }

    /// Construct an image from a byte buffer given its resolution, color model,
    /// and using the data type of the given vector.
    pub fn from_elements<T: ImageChannelType>(
        resolution: Resolution2D,
        color_model: ColorModel,
        elements: &[T],
    ) -> Self {
        let data_type = T::CHANNEL_TYPE;
        let bytes: &[u8] = bytemuck::cast_slice(elements);
        Self::from_color_model_and_bytes(
            resolution,
            color_model,
            data_type,
            re_types_core::ArrowBuffer::<u8>::from(bytes),
        )
    }

    /// Assumes RGBA, 8-bit per channel, with separate alpha.
    pub fn from_rgba32(resolution: Resolution2D, bytes: impl Into<Blob>) -> Self {
        Self::from_color_model_and_bytes(resolution, ColorModel::RGBA, ChannelDataType::U8, bytes)
    }

    /// Creates a new [`Image`] from a file.
    ///
    /// The image format will be inferred from the path (extension), or the contents if that fails.
    #[deprecated = "Use ImageEncoded::from_file instead"]
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_file_path(filepath: impl AsRef<std::path::Path>) -> std::io::Result<ImageEncoded> {
        ImageEncoded::from_file(filepath)
    }

    /// Creates a new [`Image`] from the contents of a file.
    ///
    /// If unspecified, the image format will be inferred from the contents.
    #[deprecated = "Use ImageEncoded::from_file_contents instead"]
    #[cfg(feature = "image")]
    #[inline]
    pub fn from_file_contents(
        contents: Vec<u8>,
        _format: Option<image::ImageFormat>,
    ) -> ImageEncoded {
        ImageEncoded::from_file_contents(contents)
    }
}

#[cfg(feature = "image")]
impl Image {
    /// Construct a tensor from the contents of an image file.
    ///
    /// This will spend CPU cycles decoding the image.
    /// To save CPU time and storage, we recommend you instead use
    /// [`ImageEncoded::from_file_contents`].
    ///
    /// Requires the `image` feature.
    #[inline]
    pub fn from_image_bytes(
        format: image::ImageFormat,
        file_contents: &[u8],
    ) -> Result<Self, TensorImageLoadError> {
        re_tracing::profile_function!(format!("{format:?}"));
        let image = image::load_from_memory_with_format(file_contents, format)?;
        Self::from_image(image)
    }

    /// Construct a tensor from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(image: impl Into<image::DynamicImage>) -> Result<Self, TensorImageLoadError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(image: image::DynamicImage) -> Result<Self, TensorImageLoadError> {
        re_tracing::profile_function!();

        let (w, h) = (image.width(), image.height());
        let res = Resolution2D::new(w, h);

        match image {
            image::DynamicImage::ImageLuma8(image) => {
                Ok(Self::from_elements(res, ColorModel::L, image.as_raw()))
            }
            image::DynamicImage::ImageLuma16(image) => {
                Ok(Self::from_elements(res, ColorModel::L, image.as_raw()))
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
                Ok(Self::from_elements(res, ColorModel::RGB, image.as_raw()))
            }
            image::DynamicImage::ImageRgb16(image) => {
                Ok(Self::from_elements(res, ColorModel::RGB, image.as_raw()))
            }
            image::DynamicImage::ImageRgb32F(image) => {
                Ok(Self::from_elements(res, ColorModel::RGB, image.as_raw()))
            }

            image::DynamicImage::ImageRgba8(image) => {
                Ok(Self::from_elements(res, ColorModel::RGBA, image.as_raw()))
            }
            image::DynamicImage::ImageRgba16(image) => {
                Ok(Self::from_elements(res, ColorModel::RGBA, image.as_raw()))
            }
            image::DynamicImage::ImageRgba32F(image) => {
                Ok(Self::from_elements(res, ColorModel::RGBA, image.as_raw()))
            }

            _ => {
                // It is very annoying that DynamicImage is #[non_exhaustive]
                Err(TensorImageLoadError::UnsupportedImageColorType(
                    image.color(),
                ))
            }
        }
    }
}
