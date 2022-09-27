use re_log_types::{Tensor, TensorDataTypeTrait};

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum TensorCastError {
    #[error("ndarray type mismatch with tensor storage")]
    TypeMismatch,
    #[error("tensor storage cannot be converted to ndarray")]
    UnsupportedTensorStorage,
    #[error("tensor shape did not match storage length")]
    BadTensorShape {
        #[from]
        source: ndarray::ShapeError,
    },
    #[error("unknown data store error")]
    Unknown,
}

pub fn as_ndarray<A: bytemuck::Pod + TensorDataTypeTrait>(
    tensor: &Tensor,
) -> Result<ndarray::ArrayViewD<'_, A>, TensorCastError> {
    let shape: Vec<_> = tensor.shape.iter().map(|d| d.size as usize).collect();
    let shape = ndarray::IxDyn(shape.as_slice());

    if A::DTYPE != tensor.dtype {
        return Err(TensorCastError::TypeMismatch);
    }

    ndarray::ArrayViewD::from_shape(
        shape,
        tensor
            .data
            .as_slice()
            .ok_or(TensorCastError::UnsupportedTensorStorage)?,
    )
    .map_err(|err| TensorCastError::BadTensorShape { source: err })
}

#[cfg(test)]
mod tests {
    use re_log_types::{TensorDataStore, TensorDataType, TensorDimension};

    use super::*;

    #[test]
    fn convert_tensor_to_ndarray_u8() {
        let t = Tensor {
            shape: vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            dtype: TensorDataType::U8,
            data: TensorDataStore::Dense(vec![0; 60].into()),
        };

        let n = as_ndarray::<u8>(&t).unwrap();

        println!("{:?}", n.shape());
        assert_eq!(n.shape(), &[3, 4, 5]);
    }

    #[test]
    fn convert_tensor_to_ndarray_u16() {
        let t = Tensor {
            shape: vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            dtype: TensorDataType::U16,
            data: TensorDataStore::Dense(bytemuck::pod_collect_to_vec(&vec![0_u16; 60]).into()),
        };

        let n = as_ndarray::<u16>(&t).unwrap();

        println!("{:?}", n.shape());
        assert_eq!(n.shape(), &[3, 4, 5]);
    }

    #[test]
    fn convert_tensor_to_ndarray_f32() {
        let t = Tensor {
            shape: vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            dtype: TensorDataType::F32,
            data: TensorDataStore::Dense(bytemuck::pod_collect_to_vec(&vec![0.0_f32; 60]).into()),
        };

        let n = as_ndarray::<f32>(&t).unwrap();

        println!("{:?}", n.shape());
        assert_eq!(n.shape(), &[3, 4, 5]);
    }

    #[test]
    fn check_tensor_shape_error() {
        let t = Tensor {
            shape: vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            dtype: TensorDataType::U8,
            data: TensorDataStore::Dense(vec![0; 59].into()),
        };

        let n = as_ndarray::<u8>(&t);

        assert_eq!(
            n,
            Err(TensorCastError::BadTensorShape {
                source: ndarray::ShapeError::from_kind(ndarray::ErrorKind::OutOfBounds)
            })
        );
    }

    #[test]
    fn check_tensor_type_error() {
        let t = Tensor {
            shape: vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            dtype: TensorDataType::U16,
            data: TensorDataStore::Dense(vec![0; 60].into()),
        };

        let n = as_ndarray::<u8>(&t);

        assert_eq!(n, Err(TensorCastError::TypeMismatch));
    }
}
