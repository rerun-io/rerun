use crate::{
    components::{ElementType, Resolution2D},
    datatypes::{Blob, TensorBuffer, TensorData},
    image::{find_non_empty_dim_indices, ImageConstructionError},
};

use super::DepthImage;

impl DepthImage {
    /// Try to construct a [`DepthImage`] from anything that can be converted into [`TensorData`]
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data is invalid
    /// for treating as an image.
    ///
    /// This is useful for constructing an [`DepthImage`] from an ndarray.
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

        let (blob, element_type) = match tensor_data.buffer {
            TensorBuffer::U8(buffer) => (Blob(buffer), ElementType::U8),
            TensorBuffer::U16(buffer) => (Blob(buffer.cast_to_u8()), ElementType::U16),
            TensorBuffer::U32(buffer) => (Blob(buffer.cast_to_u8()), ElementType::U32),
            TensorBuffer::U64(buffer) => (Blob(buffer.cast_to_u8()), ElementType::U64),
            TensorBuffer::I8(buffer) => (Blob(buffer.cast_to_u8()), ElementType::I8),
            TensorBuffer::I16(buffer) => (Blob(buffer.cast_to_u8()), ElementType::I16),
            TensorBuffer::I32(buffer) => (Blob(buffer.cast_to_u8()), ElementType::I32),
            TensorBuffer::I64(buffer) => (Blob(buffer.cast_to_u8()), ElementType::I64),
            TensorBuffer::F16(buffer) => (Blob(buffer.cast_to_u8()), ElementType::F16),
            TensorBuffer::F32(buffer) => (Blob(buffer.cast_to_u8()), ElementType::F32),
            TensorBuffer::F64(buffer) => (Blob(buffer.cast_to_u8()), ElementType::F64),
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
            element_type,
            draw_order: None,
            meter: None,
            colormap: None,
            point_fill_ratio: None,
        })
    }
}
