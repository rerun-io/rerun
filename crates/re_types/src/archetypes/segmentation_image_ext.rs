use crate::{
    datatypes::TensorData,
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
        let mut data: TensorData = data
            .try_into()
            .map_err(ImageConstructionError::TensorDataConversion)?;

        let non_empty_dim_inds = find_non_empty_dim_indices(&data.shape);

        match non_empty_dim_inds.len() {
            2 => {
                assign_if_none(&mut data.shape[non_empty_dim_inds[0]].name, "height");
                assign_if_none(&mut data.shape[non_empty_dim_inds[1]].name, "width");
            }
            _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
        };

        Ok(Self {
            data: data.into(),
            draw_order: None,
        })
    }
}

fn assign_if_none(name: &mut Option<::re_types_core::ArrowString>, new_name: &str) {
    if name.is_none() {
        *name = Some(new_name.into());
    }
}

// ----------------------------------------------------------------------------
// Make it possible to create an ArrayView directly from an Image.

macro_rules! forward_array_views {
    ($type:ty, $alias:ty) => {
        impl<'a> TryFrom<&'a $alias> for ::ndarray::ArrayViewD<'a, $type> {
            type Error = crate::tensor_data::TensorCastError;

            #[inline]
            fn try_from(value: &'a $alias) -> Result<Self, Self::Error> {
                (&value.data.0).try_into()
            }
        }
    };
}

forward_array_views!(u8, SegmentationImage);
forward_array_views!(u16, SegmentationImage);
forward_array_views!(u32, SegmentationImage);
forward_array_views!(u64, SegmentationImage);

forward_array_views!(i8, SegmentationImage);
forward_array_views!(i16, SegmentationImage);
forward_array_views!(i32, SegmentationImage);
forward_array_views!(i64, SegmentationImage);

forward_array_views!(half::f16, SegmentationImage);
forward_array_views!(f32, SegmentationImage);
forward_array_views!(f64, SegmentationImage);

// ----------------------------------------------------------------------------
