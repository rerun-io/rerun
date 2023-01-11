use std::sync::Arc;

use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::{msg_bundle::Component, ClassicTensor, TensorDataStore, TensorId};

pub trait TensorTrait {
    fn id(&self) -> TensorId;
    fn shape(&self) -> &[TensorDimension];
    fn num_dim(&self) -> usize;
    fn is_shaped_like_an_image(&self) -> bool;
}

#[derive(Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
pub enum TensorData {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    // ---
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    // ---
    //TODO(john) F16
    //F16(Vec<arrow2::types::f16>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TensorDimension {
    /// Number of elements on this dimension.
    /// I.e. size-1 is the maximum allowed index.
    pub size: u64,

    /// Optional name of the dimension, e.g. "color" or "width"
    pub name: Option<String>,
}

impl TensorDimension {
    const DEFAULT_NAME_WIDTH: &'static str = "width";
    const DEFAULT_NAME_HEIGHT: &'static str = "height";
    const DEFAULT_NAME_DEPTH: &'static str = "depth";

    pub fn height(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_HEIGHT))
    }

    pub fn width(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_WIDTH))
    }

    pub fn depth(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_DEPTH))
    }

    pub fn named(size: u64, name: String) -> Self {
        Self {
            size,
            name: Some(name),
        }
    }

    pub fn unnamed(size: u64) -> Self {
        Self { size, name: None }
    }
}

// TODO(jleibs) This should be extended to include things like rgb vs bgr
#[derive(Clone, Copy, Debug, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum TensorDataMeaning {
    /// Default behavior: guess based on shape
    Unknown,
    /// The data is an annotated [`crate::field_types::ClassId`] which should be
    /// looked up using the appropriate [`crate::context::AnnotationContext`]
    ClassId,
}

#[derive(Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct Tensor {
    /// Unique identifier for the tensor
    #[arrow_field(skip)]
    pub tensor_id: TensorId,

    /// Dimensionality and length
    pub shape: Vec<TensorDimension>,

    pub data: TensorData,

    /// The per-element data meaning
    /// Used to indicated if the data should be interpreted as color, class_id, etc.
    pub meaning: TensorDataMeaning,
}

impl TensorTrait for Tensor {
    fn id(&self) -> TensorId {
        self.tensor_id
    }

    fn shape(&self) -> &[TensorDimension] {
        self.shape.as_slice()
    }

    fn num_dim(&self) -> usize {
        self.shape.len()
    }

    fn is_shaped_like_an_image(&self) -> bool {
        self.num_dim() == 2
            || self.num_dim() == 3 && {
                matches!(
                    self.shape.last().unwrap().size,
                    // gray, rgb, rgba
                    1 | 3 | 4
                )
            }
    }
}

impl Component for Tensor {
    fn name() -> crate::ComponentName {
        "rerun.tensor".into()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TensorCastError {
    #[error("ndarray type mismatch with tensor storage")]
    TypeMismatch,

    #[error("tensor shape did not match storage length")]
    BadTensorShape {
        #[from]
        source: ndarray::ShapeError,
    },

    #[error("ndarray Array is not contiguous and in standard order")]
    NotContiguousStdOrder,
}

impl From<&Tensor> for ClassicTensor {
    fn from(value: &Tensor) -> Self {
        let (dtype, data) = match &value.data {
            TensorData::U8(data) => (
                crate::TensorDataType::U8,
                TensorDataStore::Dense(Arc::from(data.as_slice())),
            ),
            TensorData::U16(data) => (
                crate::TensorDataType::U16,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::U32(data) => (
                crate::TensorDataType::U32,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::U64(data) => (
                crate::TensorDataType::U64,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::I8(data) => (
                crate::TensorDataType::I8,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::I16(data) => (
                crate::TensorDataType::I16,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::I32(data) => (
                crate::TensorDataType::I32,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::I64(data) => (
                crate::TensorDataType::I64,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::F32(data) => (
                crate::TensorDataType::F32,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
            TensorData::F64(data) => (
                crate::TensorDataType::F64,
                TensorDataStore::Dense(Arc::from(bytemuck::cast_slice(data.as_slice()))),
            ),
        };

        ClassicTensor::new(
            value.tensor_id,
            value.shape.clone(),
            dtype,
            value.meaning,
            data,
        )
    }
}

macro_rules! tensor_type {
    ($type:ty, $variant:ident) => {
        impl<'a> TryFrom<&'a Tensor> for ::ndarray::ArrayViewD<'a, $type> {
            type Error = TensorCastError;

            fn try_from(value: &'a Tensor) -> Result<Self, Self::Error> {
                let shape: Vec<_> = value.shape.iter().map(|d| d.size as usize).collect();

                if let TensorData::$variant(data) = &value.data {
                    ndarray::ArrayViewD::from_shape(shape, data.as_slice())
                        .map_err(|err| TensorCastError::BadTensorShape { source: err })
                } else {
                    Err(TensorCastError::TypeMismatch)
                }
            }
        }

        impl<'a> TryFrom<::ndarray::ArrayViewD<'a, $type>> for Tensor {
            type Error = TensorCastError;

            fn try_from(view: ::ndarray::ArrayViewD<'a, $type>) -> Result<Self, Self::Error> {
                let shape = view
                    .shape()
                    .iter()
                    .map(|dim| TensorDimension {
                        size: *dim as u64,
                        name: None,
                    })
                    .collect();
                view.to_slice()
                    .ok_or(TensorCastError::NotContiguousStdOrder)
                    .map(|slice| Tensor {
                        tensor_id: TensorId::random(),
                        shape,
                        data: TensorData::$variant(slice.into()),
                        meaning: TensorDataMeaning::Unknown,
                    })
            }
        }

        impl<D: ::ndarray::Dimension> TryFrom<::ndarray::Array<$type, D>> for Tensor {
            type Error = TensorCastError;

            fn try_from(value: ndarray::Array<$type, D>) -> Result<Self, Self::Error> {
                let shape = value
                    .shape()
                    .iter()
                    .map(|dim| TensorDimension {
                        size: *dim as u64,
                        name: None,
                    })
                    .collect();
                value
                    .is_standard_layout()
                    .then(|| Tensor {
                        tensor_id: TensorId::random(),
                        shape,
                        data: TensorData::$variant(value.into_raw_vec()),
                        meaning: TensorDataMeaning::Unknown,
                    })
                    .ok_or(TensorCastError::NotContiguousStdOrder)
            }
        }
    };
}

tensor_type!(u8, U8);
tensor_type!(u16, U16);
tensor_type!(u32, U32);
tensor_type!(u64, U64);

tensor_type!(i8, I8);
tensor_type!(i16, I16);
tensor_type!(i32, I32);
tensor_type!(i64, I64);

tensor_type!(f32, F32);
tensor_type!(f64, F64);

#[cfg(feature = "disabled")]
#[test]
fn test_ndarray() {
    let t0 = Tensor {
        shape: vec![
            TensorDimension {
                size: 2,
                name: None,
            },
            TensorDimension {
                size: 2,
                name: None,
            },
        ],
        data: TensorData::U16(vec![1, 2, 3, 4]),
    };
    let a0: ndarray::ArrayViewD<'_, u16> = (&t0).try_into().unwrap();
    dbg!(a0);

    let a = ndarray::Array3::<f64>::zeros((1, 2, 3));
    let t1 = Tensor::try_from(a.into_dyn().view());
    dbg!(t1);
}

#[test]
fn test_arrow() {
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let tensors_in = vec![
        Tensor {
            tensor_id: TensorId(std::default::Default::default()),
            shape: vec![TensorDimension {
                size: 4,
                name: None,
            }],
            data: TensorData::U16(vec![1, 2, 3, 4]),
            meaning: TensorDataMeaning::Unknown,
        },
        Tensor {
            tensor_id: TensorId(std::default::Default::default()),
            shape: vec![TensorDimension {
                size: 2,
                name: None,
            }],
            data: TensorData::F32(vec![1.23, 2.45]),
            meaning: TensorDataMeaning::Unknown,
        },
    ];

    let array: Box<dyn arrow2::array::Array> = tensors_in.iter().try_into_arrow().unwrap();
    let tensors_out: Vec<Tensor> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(tensors_in, tensors_out);
}
