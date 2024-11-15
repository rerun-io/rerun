use crate::{
    datatypes::{ImageFormat, TensorData},
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
        let shape = tensor_data.shape;

        let non_empty_dim_inds = find_non_empty_dim_indices(&shape);

        if non_empty_dim_inds.len() != 2 {
            return Err(ImageConstructionError::BadImageShape(shape));
        }

        let (blob, datatype) = blob_and_datatype_from_tensor(tensor_data.buffer);

        let (height, width) = (&shape[non_empty_dim_inds[0]], &shape[non_empty_dim_inds[1]]);
        let height = height.size as u32;
        let width = width.size as u32;

        let image_format = ImageFormat::depth([width, height], datatype);

        Ok(Self {
            buffer: blob.into(),
            format: image_format.into(),
            draw_order: None,
            meter: None,
            colormap: None,
            point_fill_ratio: None,
            depth_range: None,
        })
    }
}
