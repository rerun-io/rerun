use crate::{
    components::{ChannelDataType, Resolution2D},
    datatypes::{Blob, TensorBuffer, TensorData},
    image::{find_non_empty_dim_indices, ImageConstructionError},
};

use super::SegmentationImage;

impl SegmentationImage {
    /// Try to construct a [`SegmentationImage`] from anything that can be converted into [`TensorData`]
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data is invalid
    /// for treating as an image.
    ///
    /// This is useful for constructing an [`SegmentationImage`] from an ndarray.
    pub fn try_from<T: TryInto<TensorData>>(data: T) -> Result<Self, ImageConstructionError<T>>
    where
        <T as TryInto<TensorData>>::Error: std::error::Error,
    {
        let tensor_data: TensorData = data
            .try_into()
            .map_err(ImageConstructionError::TensorDataConversion)?;

        let non_empty_dim_inds = find_non_empty_dim_indices(&tensor_data.shape);

        if non_empty_dim_inds.len() != 2 {
            return Err(ImageConstructionError::BadImageShape(tensor_data.shape));
        }

        let (blob, data_type) = match tensor_data.buffer {
            TensorBuffer::U8(buffer) => (Blob(buffer), ChannelDataType::U8),
            TensorBuffer::U16(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::U16),
            TensorBuffer::U32(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::U32),
            TensorBuffer::U64(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::U64),
            TensorBuffer::I8(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::I8),
            TensorBuffer::I16(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::I16),
            TensorBuffer::I32(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::I32),
            TensorBuffer::I64(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::I64),
            TensorBuffer::F16(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::F16),
            TensorBuffer::F32(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::F32),
            TensorBuffer::F64(buffer) => (Blob(buffer.cast_to_u8()), ChannelDataType::F64),
            TensorBuffer::Nv12(_) | TensorBuffer::Yuy2(_) => {
                return Err(ImageConstructionError::ChromaDownsamplingNotSupported);
            }
        };

        let (height, width) = (
            &tensor_data.shape[non_empty_dim_inds[0]],
            &tensor_data.shape[non_empty_dim_inds[1]],
        );
        let height = height.size as u32;
        let width = width.size as u32;
        let resolution = Resolution2D::from([width, height]);

        Ok(Self {
            data: blob.into(),
            resolution,
            data_type,
            draw_order: None,
            opacity: None,
        })
    }
}
