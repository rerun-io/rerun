use crate::{
    datatypes::{ImageFormat, TensorData},
    image::{blob_and_datatype_from_tensor, find_non_empty_dim_indices, ImageConstructionError},
};

use super::SegmentationImage;

impl SegmentationImage {
    /// Try to construct a [`SegmentationImage`] from anything that can be converted into [`TensorData`]
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data is invalid
    /// for treating as an image.
    ///
    /// This is useful for constructing a [`SegmentationImage`] from an ndarray.
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

        let image_format = ImageFormat::segmentation([width as _, height as _], datatype);

        Ok(Self {
            buffer: blob.into(),
            format: image_format.into(),
            draw_order: None,
            opacity: None,
        })
    }
}
