use super::Image;
use crate::components::ImageBuffer;
use crate::datatypes::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat, TensorData};
use crate::image::{
    ImageChannelType, ImageConstructionError, blob_and_datatype_from_tensor,
    find_non_empty_dim_indices,
};
#[cfg(feature = "image")]
use crate::image::{ImageConversionError, ImageLoadError};

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

        let image_format =
            ImageFormat::from_color_model([width as _, height as _], color_model, datatype);

        Ok(Self::new(blob, image_format))
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
                "Expected {width}x{height} {pixel_format:?} image to be {num_expected_bytes} B, but got {} B",
                buffer.len()
            );
        }

        Self::new(buffer, image_format)
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
        let image_format = ImageFormat::from_color_model([width, height], color_model, datatype);

        let num_expected_bytes = image_format.num_bytes();
        if buffer.len() != num_expected_bytes {
            re_log::warn_once!(
                "Expected {width}x{height} {color_model:?} {datatype} image to be {num_expected_bytes} B, but got {} B",
                buffer.len()
            );
        }

        Self::new(buffer, image_format)
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
        Self::from_color_model_and_bytes(bytes, [width, height], color_model, datatype)
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
}

#[cfg(feature = "image")]
impl Image {
    /// Construct an image from the contents of an image file.
    ///
    /// This will spend CPU cycles decoding the image.
    /// To save CPU time and storage, we recommend you instead use
    /// [`super::EncodedImage::from_file_contents`].
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

    /// Construct an image from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(image: impl Into<image::DynamicImage>) -> Result<Self, ImageConversionError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct an image from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(image: image::DynamicImage) -> Result<Self, ImageConversionError> {
        let (image_buffer, image_format) = ImageBuffer::from_image(image)?;
        Ok(Self::new(image_buffer, image_format))
    }
}
