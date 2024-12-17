use crate::datatypes::TensorData;

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

        Ok(Self {
            data: data.into(),
            value_range: None,
        })
    }

    /// Update the `names` of the contained [`TensorData`] dimensions.
    ///
    /// Any existing names will be overwritten.
    ///
    /// If the wrong number of names are given, a warning will be logged,
    /// and the names might not show up correctly.
    pub fn with_dim_names(
        mut self,
        names: impl IntoIterator<Item = impl Into<ArrowString>>,
    ) -> Self {
        self.data.0 = self.data.0.with_dim_names(names);
        self
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
        TensorData::from_image(image).map(|data| Self {
            data: data.into(),
            value_range: None,
        })
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(
        image: image::DynamicImage,
    ) -> Result<Self, crate::tensor_data::TensorImageLoadError> {
        TensorData::from_dynamic_image(image).map(|data| Self {
            data: data.into(),
            value_range: None,
        })
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
