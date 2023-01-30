//! Helper used to access `re_log_types::Tensor` as an ndarray
//!
//! This exposes an Array *view* while using the underlying `TensorDataStore`.
//! This is particularly helpful for performing slice-operations for
//! dimensionality reduction.

use re_log_types::{component_types, ClassicTensor, TensorDataStore, TensorDataTypeTrait};

pub mod dimension_mapping;

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

    #[error("wrong number of names")]
    BadNamesLength,

    #[error("unsupported datatype - only numeric datatypes are supported")]
    UnsupportedDataType,
}

pub fn as_ndarray<A: bytemuck::Pod + TensorDataTypeTrait>(
    tensor: &ClassicTensor,
) -> Result<ndarray::ArrayViewD<'_, A>, TensorCastError> {
    let shape: Vec<_> = tensor.shape().iter().map(|d| d.size as usize).collect();
    let shape = ndarray::IxDyn(shape.as_slice());

    if A::DTYPE != tensor.dtype() {
        return Err(TensorCastError::TypeMismatch);
    }

    ndarray::ArrayViewD::from_shape(
        shape,
        tensor
            .data()
            .ok_or(TensorCastError::UnsupportedTensorStorage)?,
    )
    .map_err(|err| TensorCastError::BadTensorShape { source: err })
}

pub fn to_rerun_tensor<A: ndarray::Data + ndarray::RawData, D: ndarray::Dimension>(
    data: &ndarray::ArrayBase<A, D>,
    names: Option<Vec<String>>,
    meaning: component_types::TensorDataMeaning,
) -> Result<ClassicTensor, TensorCastError>
where
    <A as ndarray::RawData>::Elem: TensorDataTypeTrait + bytemuck::Pod,
{
    // TODO(emilk): fewer memory allocations here.
    let vec: Vec<_> = data.iter().cloned().collect();
    let vec = bytemuck::allocation::try_cast_vec(vec)
        .unwrap_or_else(|(_err, vec)| bytemuck::allocation::pod_collect_to_vec(&vec));

    let arc = std::sync::Arc::from(vec);

    let shape = if let Some(names) = names {
        if names.len() != data.shape().len() {
            return Err(TensorCastError::BadNamesLength);
        }

        data.shape()
            .iter()
            .zip(names)
            .map(|(&d, name)| component_types::TensorDimension::named(d as _, name))
            .collect()
    } else {
        data.shape()
            .iter()
            .map(|&d| component_types::TensorDimension::unnamed(d as _))
            .collect()
    };

    Ok(ClassicTensor::new(
        component_types::TensorId::random(),
        shape,
        A::Elem::DTYPE,
        meaning,
        TensorDataStore::Dense(arc),
    ))
}

#[cfg(test)]
mod tests {
    use re_log_types::{
        component_types::{TensorDataMeaning, TensorDimension, TensorId},
        TensorDataStore, TensorDataType,
    };

    use super::*;

    #[test]
    fn convert_tensor_to_ndarray_u8() {
        let t = ClassicTensor::new(
            TensorId::random(),
            vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            TensorDataType::U8,
            TensorDataMeaning::Unknown,
            TensorDataStore::Dense(vec![0; 60].into()),
        );

        let n = as_ndarray::<u8>(&t).unwrap();

        assert_eq!(n.shape(), &[3, 4, 5]);
    }

    #[test]
    fn convert_tensor_to_ndarray_u16() {
        let t = ClassicTensor::new(
            TensorId::random(),
            vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            TensorDataType::U16,
            TensorDataMeaning::Unknown,
            TensorDataStore::Dense(bytemuck::pod_collect_to_vec(&[0_u16; 60]).into()),
        );

        let n = as_ndarray::<u16>(&t).unwrap();

        assert_eq!(n.shape(), &[3, 4, 5]);
    }

    #[test]
    fn convert_tensor_to_ndarray_f32() {
        let t = ClassicTensor::new(
            TensorId::random(),
            vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            TensorDataType::F32,
            component_types::TensorDataMeaning::Unknown,
            TensorDataStore::Dense(bytemuck::pod_collect_to_vec(&[0_f32; 60]).into()),
        );

        let n = as_ndarray::<f32>(&t).unwrap();

        assert_eq!(n.shape(), &[3, 4, 5]);
    }

    #[test]
    fn convert_ndarray_u8_to_tensor() {
        let n = ndarray::array![[1., 2., 3.], [4., 5., 6.]];
        let t = to_rerun_tensor(
            &n,
            Some(vec!["height".to_owned(), "width".to_owned()]),
            TensorDataMeaning::Unknown,
        )
        .unwrap();

        assert_eq!(
            t.shape(),
            &[TensorDimension::height(2), TensorDimension::width(3)]
        );
    }

    #[test]
    fn convert_ndarray_slice_to_tensor() {
        let n = ndarray::array![[1., 2., 3.], [4., 5., 6.]];
        let n = &n.slice(ndarray::s![.., 1]);
        let t = to_rerun_tensor(
            n,
            Some(vec!["height".to_owned()]),
            TensorDataMeaning::Unknown,
        )
        .unwrap();

        assert_eq!(t.shape(), &[TensorDimension::height(2)]);
    }

    #[test]
    fn check_slices() {
        let t = ClassicTensor::new(
            TensorId::random(),
            vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            TensorDataType::U16,
            TensorDataMeaning::Unknown,
            TensorDataStore::Dense(
                bytemuck::pod_collect_to_vec(&(0..60).map(|x| x as i16).collect::<Vec<i16>>())
                    .into(),
            ),
        );

        let n = as_ndarray::<u16>(&t).unwrap();

        // First element should be 0
        assert_eq!(n[[0, 0, 0]], 0);
        // Last element should be 59
        assert_eq!(n[[2, 3, 4]], 59);

        // Try all the indices:
        for z in 0..3 {
            for y in 0..4 {
                for x in 0..5 {
                    assert_eq!(n[[z, y, x]] as usize, z * 4 * 5 + y * 5 + x);
                }
            }
        }

        // Slice the tensor
        let sl: ndarray::ArrayView2<'_, u16> = n.slice(ndarray::s![.., 1, ..]);

        // New slice is missing the middle dimension
        assert_eq!(sl.shape(), &[3, 5]);

        // Equivalent to (0,1,0) = 0*20 + 1*5 + 0 = 5
        assert_eq!(sl[[0, 0]], 5);
        // Equivalent to (1,1,3) = 1*20 + 1*5 + 3 = 28
        assert_eq!(sl[[1, 3]], 28);
    }

    #[test]
    fn check_tensor_shape_error() {
        let t = ClassicTensor::new(
            TensorId::random(),
            vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            TensorDataType::U8,
            TensorDataMeaning::Unknown,
            TensorDataStore::Dense(vec![0; 59].into()),
        );

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
        let t = ClassicTensor::new(
            TensorId::random(),
            vec![
                TensorDimension::unnamed(3),
                TensorDimension::unnamed(4),
                TensorDimension::unnamed(5),
            ],
            TensorDataType::U16,
            TensorDataMeaning::Unknown,
            TensorDataStore::Dense(vec![0; 60].into()),
        );

        let n = as_ndarray::<u8>(&t);

        assert_eq!(n, Err(TensorCastError::TypeMismatch));
    }
}
