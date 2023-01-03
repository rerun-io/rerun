use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

#[derive(Debug, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
pub enum TensorData {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
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

#[derive(Debug, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct Tensor {
    /// Dimensionality and length
    pub shape: Vec<TensorDimension>,

    pub data: TensorData,
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
                        shape,
                        data: TensorData::$variant(slice.into()),
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
                        shape,
                        data: TensorData::$variant(value.into_raw_vec()),
                    })
                    .ok_or(TensorCastError::NotContiguousStdOrder)
            }
        }
    };
}

tensor_type!(u8, U8);
tensor_type!(u16, U16);
tensor_type!(u32, U32);
tensor_type!(f32, F32);
tensor_type!(f64, F64);

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
    use arrow2_convert::serialize::TryIntoArrow;
    let x: Box<dyn arrow2::array::Array> = [
        Tensor {
            shape: vec![TensorDimension {
                size: 4,
                name: None,
            }],
            data: TensorData::U16(vec![1, 2, 3, 4]),
        },
        Tensor {
            shape: vec![TensorDimension {
                size: 2,
                name: None,
            }],
            data: TensorData::F32(vec![1.23, 2.45]),
        },
    ]
    .try_into_arrow()
    .unwrap();

    let table = re_format::arrow::format_table(&[x], ["x"]);
    println!("{table}");
}
