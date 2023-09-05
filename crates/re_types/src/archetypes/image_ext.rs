use smallvec::SmallVec;

use crate::{
    datatypes::{TensorData, TensorDimension},
    image::ImageConstructionError,
};

use super::Image;

impl Image {
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
            3 => match data.shape[non_empty_dim_inds[2]].size {
                3 | 4 => {
                    data.shape[non_empty_dim_inds[0]].name = Some("height".into());
                    data.shape[non_empty_dim_inds[1]].name = Some("width".into());
                    data.shape[non_empty_dim_inds[2]].name = Some("depth".into());
                }
                _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
            },
            _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
        };

        Ok(Self {
            data: data.into(),
            draw_order: None,
        })
    }

    pub fn with_id(self, id: crate::datatypes::TensorId) -> Self {
        Self {
            data: TensorData {
                id,
                shape: self.data.0.shape,
                buffer: self.data.0.buffer,
            }
            .into(),
            draw_order: None,
        }
    }
}

// Returns the indices of an appropriate set of non-empty dimensions
fn find_non_empty_dim_indices(shape: &Vec<TensorDimension>) -> SmallVec<[usize; 4]> {
    if shape.len() < 2 {
        return SmallVec::<_>::new();
    }

    let mut iter_non_empty =
        shape
            .iter()
            .enumerate()
            .filter_map(|(ind, dim)| if dim.size != 1 { Some(ind) } else { None });

    // 0 must be valid since shape isn't empty or we would have returned an Err above
    let mut first_non_empty = iter_non_empty.next().unwrap_or(0);
    let mut last_non_empty = iter_non_empty.last().unwrap_or(first_non_empty);

    // Note, these are inclusive ranges.

    // First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    // Grow to a min-size of 2.
    // (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while last_non_empty - first_non_empty < 1 && last_non_empty < (shape.len() - 1) {
        last_non_empty += 1;
    }

    // Next, consider empty outer dimensions if we still need them.
    // Grow up to 3 if the inner dimension is already 3 or 4 (Color Images)
    // Otherwise, only grow up to 2.
    // (1x1x3) -> 1x1x3 rgb rather than 1x3 mono
    let target = match shape[last_non_empty].size {
        3 | 4 => 2,
        _ => 1,
    };

    while last_non_empty - first_non_empty < target && first_non_empty > 0 {
        first_non_empty -= 1;
    }

    (first_non_empty..=last_non_empty).collect()
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

forward_array_views!(u8, Image);
forward_array_views!(u16, Image);
forward_array_views!(u32, Image);
forward_array_views!(u64, Image);

forward_array_views!(i8, Image);
forward_array_views!(i16, Image);
forward_array_views!(i32, Image);
forward_array_views!(i64, Image);

// TODO(jleibs): F16 Support
//forward_array_views!(half::f16, Image);
forward_array_views!(f32, Image);
forward_array_views!(f64, Image);

// ----------------------------------------------------------------------------
