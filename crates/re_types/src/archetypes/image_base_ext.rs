use smallvec::SmallVec;

use crate::datatypes::{ImageVariant, TensorDimension};

use super::ImageBase;

// ----------------------------------------------------------------------------
impl ImageBase {
    pub fn with_id(self, id: crate::datatypes::TensorId) -> Self {
        Self {
            variant: self.variant,
            data: crate::datatypes::TensorData {
                id,
                shape: self.data.0.shape,
                buffer: self.data.0.buffer,
            }
            .into(),
        }
    }
}
// ----------------------------------------------------------------------------

/// An Monochrome, RGB, or RGBA Image
///
/// This is an alias for the [`ImageBase`] archetype which correctly populates
/// [`ImageVariant`] component based on the provided [`TensorData`]
#[derive(Clone, Debug, PartialEq)]
pub struct Image(ImageBase);

impl Image {
    pub const NUM_COMPONENTS: usize = ImageBase::NUM_COMPONENTS;

    pub fn base(&self) -> &ImageBase {
        &self.0
    }

    pub fn data(&self) -> &crate::datatypes::TensorData {
        &self.base().data.0
    }
}

impl crate::Archetype for Image {
    #[inline]
    fn name() -> crate::ArchetypeName {
        crate::ArchetypeName::Borrowed("rerun.archetypes.Tensor")
    }

    #[inline]
    fn required_components() -> &'static [crate::ComponentName] {
        ImageBase::recommended_components()
    }

    #[inline]
    fn recommended_components() -> &'static [crate::ComponentName] {
        ImageBase::recommended_components()
    }

    #[inline]
    fn optional_components() -> &'static [crate::ComponentName] {
        ImageBase::optional_components()
    }

    #[inline]
    fn all_components() -> &'static [crate::ComponentName] {
        ImageBase::all_components()
    }

    #[inline]
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        self.0.try_to_arrow()
    }

    #[inline]
    fn try_from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        let base = ImageBase::try_from_arrow(arrow_data)?;

        let non_empty_dim_inds = find_non_empty_dim_indices(&base.data.0.shape);

        let variant = base.variant.0;
        let dims = non_empty_dim_inds.len();
        let last_dim_size = non_empty_dim_inds
            .last()
            .map_or(0, |i| base.data.0.shape[*i].size);

        match (variant, dims, last_dim_size) {
            (ImageVariant::Mono(_), 2, _)
            | (ImageVariant::Rgb(_), 3, 3)
            | (ImageVariant::Rgba(_), 3, 4) => Ok(Self(base)),
            _ => Err(crate::DeserializationError::ValidationError(format!(
                "Invalid ImageBase for Image. Variant: {:?}, Shape: {:?}",
                base.variant.0, base.data.0.shape
            ))),
        }
    }
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageConstructionError<T: TryInto<crate::datatypes::TensorData>> {
    #[error("Could not convert source to TensorData")]
    TensorDataConversion(T::Error),

    #[error("Could not create Image from TensorData with shape {0:?}")]
    BadImageShape(Vec<TensorDimension>),
}

impl Image {
    /// Try to construct an [`Image`] from anything that can be converted into [`TensorData`]
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data is invalid
    /// for treating as an image.
    ///
    /// This is useful for constructing a tensor from an ndarray.
    pub fn try_from<T: TryInto<crate::datatypes::TensorData>>(
        data: T,
    ) -> Result<Self, ImageConstructionError<T>> {
        let mut data: crate::datatypes::TensorData = data
            .try_into()
            .map_err(ImageConstructionError::TensorDataConversion)?;

        let non_empty_dim_inds = find_non_empty_dim_indices(&data.shape);

        let variant = match non_empty_dim_inds.len() {
            2 => {
                data.shape[non_empty_dim_inds[0]].name = Some("height".into());
                data.shape[non_empty_dim_inds[1]].name = Some("width".into());
                ImageVariant::Mono(true)
            }
            3 => match data.shape[non_empty_dim_inds[2]].size {
                3 => {
                    data.shape[non_empty_dim_inds[0]].name = Some("height".into());
                    data.shape[non_empty_dim_inds[1]].name = Some("width".into());
                    data.shape[non_empty_dim_inds[2]].name = Some("color".into());
                    ImageVariant::Rgb(true)
                }
                4 => {
                    data.shape[non_empty_dim_inds[0]].name = Some("height".into());
                    data.shape[non_empty_dim_inds[1]].name = Some("width".into());
                    data.shape[non_empty_dim_inds[2]].name = Some("color".into());
                    ImageVariant::Rgba(true)
                }
                _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
            },
            _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
        };

        Ok(Self(ImageBase {
            variant: variant.into(),
            data: data.into(),
        }))
    }

    pub fn with_id(self, id: crate::datatypes::TensorId) -> Self {
        Self(self.0.with_id(id))
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
            type Error = crate::datatypes::TensorCastError;

            #[inline]
            fn try_from(value: &'a $alias) -> Result<Self, Self::Error> {
                value.data().try_into()
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
