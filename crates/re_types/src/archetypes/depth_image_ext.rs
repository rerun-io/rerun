use crate::{
    datatypes::TensorData,
    image::{find_non_empty_dim_indices, ImageConstructionError},
};

use super::DepthImage;

impl DepthImage {
    /// Try to construct an [`Image`] from anything that can be converted into [`TensorData`]
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data is invalid
    /// for treating as an image.
    ///
    /// This is useful for constructing an [`Image`] from an ndarray.
    pub fn try_from<T: TryInto<TensorData>>(data: T) -> Result<Self, ImageConstructionError<T>> {
        let mut data: TensorData = data
            .try_into()
            .map_err(ImageConstructionError::TensorDataConversion)?;

        let non_empty_dim_inds = find_non_empty_dim_indices(&data.shape);

        match non_empty_dim_inds.len() {
            2 => {
                data.shape[non_empty_dim_inds[0]].name = Some("height".into());
                data.shape[non_empty_dim_inds[1]].name = Some("width".into());
            }
            _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
        };

        Ok(Self {
            data: data.into(),
            draw_order: None,
            meter: None,
        })
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

forward_array_views!(u8, DepthImage);
forward_array_views!(u16, DepthImage);
forward_array_views!(u32, DepthImage);
forward_array_views!(u64, DepthImage);

forward_array_views!(i8, DepthImage);
forward_array_views!(i16, DepthImage);
forward_array_views!(i32, DepthImage);
forward_array_views!(i64, DepthImage);

// TODO(jleibs): F16 Support
//forward_array_views!(half::f16, Image);
forward_array_views!(f32, DepthImage);
forward_array_views!(f64, DepthImage);

// ----------------------------------------------------------------------------
