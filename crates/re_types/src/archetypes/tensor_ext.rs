use crate::{
    datatypes::{TensorData, TensorDimension, TensorId},
    ArrowString,
};

use super::Tensor;

impl Tensor {
    /// Try to construct a [`Tensor`] from anything that can be converted into [`TensorData`]
    ///
    /// This is useful for constructing a tensor from an ndarray.
    pub fn try_from<T: TryInto<TensorData>>(data: T) -> Result<Self, T::Error> {
        let data: crate::datatypes::TensorData = data.try_into()?;

        Ok(Self { data: data.into() })
    }

    /// Replace the `id` of the contained [`TensorData`] with a new [`TensorId`]
    pub fn with_id(self, id: TensorId) -> Self {
        Self {
            data: TensorData {
                id,
                shape: self.data.0.shape,
                buffer: self.data.0.buffer,
            }
            .into(),
        }
    }

    /// Update the `names` of the contained [`TensorData`] dimensions.
    ///
    /// Any existing Dimension names will be be overwritten.
    ///
    /// If too many, or too few names are provided, this function will warn and only
    /// update the subset of names that it can.
    pub fn with_names(self, names: impl IntoIterator<Item = impl Into<ArrowString>>) -> Self {
        let names: Vec<_> = names.into_iter().map(|x| Some(x.into())).collect();
        if names.len() != self.data.0.shape.len() {
            re_log::warn_once!(
                "Wrong number of names provided for tensor dimension. {} provided but {} expected.",
                names.len(),
                self.data.0.shape.len(),
            );
        }
        Self {
            data: crate::datatypes::TensorData {
                id: self.data.0.id,
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
