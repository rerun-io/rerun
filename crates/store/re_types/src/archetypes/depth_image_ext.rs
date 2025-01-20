use crate::{
    components::{ImageBuffer, ImageFormat},
    datatypes::{ChannelDatatype, ColorModel, TensorData},
    image::{blob_and_datatype_from_tensor, find_non_empty_dim_indices, ImageConstructionError},
};

use super::DepthImage;

impl DepthImage {
    /// Try to construct a [`DepthImage`] from anything that can be converted into [`TensorData`]
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data is invalid
    /// for treating as an image.
    ///
    /// This is useful for constructing a [`DepthImage`] from an ndarray.
    pub fn try_from<T: TryInto<TensorData>>(data: T) -> Result<Self, ImageConstructionError<T>>
    where
        <T as TryInto<TensorData>>::Error: std::error::Error,
    {
        let tensor_data: TensorData = data
            .try_into()
            .map_err(ImageConstructionError::TensorDataConversion)?;
        let TensorData { shape, buffer, .. } = tensor_data;

        let non_empty_dim_inds = find_non_empty_dim_indices(&shape);

        if non_empty_dim_inds.len() != 2 {
            return Err(ImageConstructionError::BadImageShape(shape));
        }

        let (blob, datatype) = blob_and_datatype_from_tensor(buffer);

        let (height, width) = (shape[non_empty_dim_inds[0]], shape[non_empty_dim_inds[1]]);

        let image_format = ImageFormat::depth([width as u32, height as u32], datatype);

        Ok(Self::new(blob, image_format))
    }

    /// Construct a depth image from a byte buffer given its resolution, and data type.
    pub fn from_data_type_and_bytes(
        bytes: impl Into<ImageBuffer>,
        [width, height]: [u32; 2],
        datatype: ChannelDatatype,
    ) -> Self {
        let buffer = bytes.into();

        let image_format = ImageFormat::depth([width, height], datatype);

        let num_expected_bytes = image_format.num_bytes();
        if buffer.len() != num_expected_bytes {
            re_log::warn_once!(
                "Expected {width}x{height} {} {datatype:?} image to be {num_expected_bytes} B, but got {} B", ColorModel::L, buffer.len()
            );
        }

        Self::new(buffer, image_format)
    }

    /// From an 16-bit grayscale image.
    pub fn from_gray16(bytes: impl Into<ImageBuffer>, resolution: [u32; 2]) -> Self {
        Self::from_data_type_and_bytes(bytes, resolution, ChannelDatatype::U16)
    }
}
