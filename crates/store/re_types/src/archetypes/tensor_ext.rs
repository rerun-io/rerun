use crate::datatypes::{TensorData, TensorDimension};

use re_types_core::ArrowString;

use super::Tensor;

impl Tensor {
    /// Accessor to the underlying [`TensorData`].
    pub fn data(&self) -> &TensorData {
        &self.data.0
    }

    /// Try to construct a [`Tensor`] from anything that can be converted into [`TensorData`]
    ///
    /// This is useful for constructing a tensor from an ndarray.
    pub fn try_from<T: TryInto<TensorData>>(data: T) -> Result<Self, T::Error> {
        let data: TensorData = data.try_into()?;

        Ok(Self { data: data.into() })
    }

    /// Update the `names` of the contained [`TensorData`] dimensions.
    ///
    /// Any existing Dimension names will be overwritten.
    ///
    /// If too many, or too few names are provided, this function will warn and only
    /// update the subset of names that it can.
    pub fn with_dim_names(self, names: impl IntoIterator<Item = impl Into<ArrowString>>) -> Self {
        let names: Vec<_> = names.into_iter().map(|x| Some(x.into())).collect();
        if names.len() != self.data.0.shape.len() {
            re_log::warn_once!(
                "Wrong number of names provided for tensor dimension. {} provided but {} expected.",
                names.len(),
                self.data.0.shape.len(),
            );
        }
        Self {
            data: TensorData {
                shape: self
                    .data
                    .0
                    .shape
                    .into_iter()
                    .zip(names.into_iter().chain(std::iter::repeat(None)))
                    .map(|(dim, name)| TensorDimension {
                        size: dim.size,
                        name: name.or(dim.name),
                    })
                    .collect(),
                buffer: self.data.0.buffer,
            }
            .into(),
        }
    }
}

#[cfg(feature = "image")]
impl Tensor {
    /// Construct a tensor from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(
        image: impl Into<image::DynamicImage>,
    ) -> Result<Self, crate::tensor_data::TensorImageLoadError> {
        TensorData::from_image(image).map(|data| Self { data: data.into() })
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(
        image: image::DynamicImage,
    ) -> Result<Self, crate::tensor_data::TensorImageLoadError> {
        TensorData::from_dynamic_image(image).map(|data| Self { data: data.into() })
    }
}

impl AsRef<TensorData> for Tensor {
    #[inline(always)]
    fn as_ref(&self) -> &TensorData {
        &self.data
    }
}

impl std::ops::Deref for Tensor {
    type Target = TensorData;

    #[inline(always)]
    fn deref(&self) -> &TensorData {
        &self.data
    }
}

impl std::borrow::Borrow<TensorData> for Tensor {
    #[inline(always)]
    fn borrow(&self) -> &TensorData {
        &self.data
    }
}

// ----------------------------------------------------------------------------
// Make it possible to create an ArrayView directly from a Tensor.

macro_rules! forward_array_views {
    ($type:ty, $alias:ty) => {
        impl<'a> TryFrom<&'a $alias> for ::ndarray::ArrayViewD<'a, $type> {
            type Error = crate::tensor_data::TensorCastError;

            #[inline]
            fn try_from(value: &'a $alias) -> Result<Self, Self::Error> {
                value.data().try_into()
            }
        }
    };
}

forward_array_views!(u8, Tensor);
forward_array_views!(u16, Tensor);
forward_array_views!(u32, Tensor);
forward_array_views!(u64, Tensor);

forward_array_views!(i8, Tensor);
forward_array_views!(i16, Tensor);
forward_array_views!(i32, Tensor);
forward_array_views!(i64, Tensor);

forward_array_views!(half::f16, Tensor);
forward_array_views!(f32, Tensor);
forward_array_views!(f64, Tensor);
