use crate::datatypes::{TensorData, TensorDimension};

#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageConstructionError<T: TryInto<TensorData>> {
    #[error("Could not convert source to TensorData")]
    TensorDataConversion(T::Error),

    #[error("Could not create Image from TensorData with shape {0:?}")]
    BadImageShape(Vec<TensorDimension>),
}
