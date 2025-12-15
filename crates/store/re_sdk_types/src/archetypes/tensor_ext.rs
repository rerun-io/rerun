use re_types_core::{ArrowString, Loggable as _, try_serialize_field};

use super::Tensor;
use crate::components;
use crate::datatypes::TensorData;

impl Tensor {
    /// Try to construct a [`Tensor`] from anything that can be converted into [`TensorData`]
    ///
    /// This is useful for constructing a tensor from an ndarray.
    pub fn try_from<T: TryInto<TensorData>>(data: T) -> Result<Self, T::Error> {
        let data: TensorData = data.try_into()?;
        Ok(Self::new(data))
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
        if let Some(data) = self.data.take() {
            match components::TensorData::from_arrow(&data.array) {
                Ok(tensor_data) => {
                    if tensor_data.len() > 1 {
                        re_log::warn!(
                            "Can't set dimension names on a tensor archetype with multiple tensor data instances."
                        );
                        return self;
                    }
                    let Some(tensor_data) = tensor_data.into_iter().next() else {
                        re_log::warn!(
                            "Can't set dimension names on a tensor archetype without any tensor data instances."
                        );
                        return self;
                    };

                    self.data = try_serialize_field::<components::TensorData>(
                        Self::descriptor_data(),
                        [components::TensorData(tensor_data.0.with_dim_names(names))],
                    );
                }
                Err(err) => re_log::warn!(
                    "Failed to read arrow tensor data: {}",
                    re_error::format_ref(&err)
                ),
            }
        } else {
            re_log::warn!("Can't set names on a tensor that doesn't have any data");
        }
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
        TensorData::from_image(image).map(Self::new)
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(
        image: image::DynamicImage,
    ) -> Result<Self, crate::tensor_data::TensorImageLoadError> {
        TensorData::from_dynamic_image(image).map(Self::new)
    }
}
