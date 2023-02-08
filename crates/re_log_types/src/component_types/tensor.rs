use std::sync::Arc;

use arrow2::array::{FixedSizeBinaryArray, MutableFixedSizeBinaryArray};
use arrow2::buffer::Buffer;
use arrow2_convert::deserialize::ArrowDeserialize;
use arrow2_convert::field::ArrowField;
use arrow2_convert::{serialize::ArrowSerialize, ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::TensorElement;
use crate::{msg_bundle::Component, ClassicTensor, TensorDataStore};

pub trait TensorTrait {
    fn id(&self) -> TensorId;
    fn shape(&self) -> &[TensorDimension];
    fn num_dim(&self) -> usize;
    fn is_shaped_like_an_image(&self) -> bool;
    fn is_vector(&self) -> bool;
    fn meaning(&self) -> TensorDataMeaning;
    fn get(&self, index: &[u64]) -> Option<TensorElement>;
}

// ----------------------------------------------------------------------------

/// A unique id per [`Tensor`].
///
/// TODO(emilk): this should be a hash of the tensor (CAS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TensorId(pub uuid::Uuid);

impl nohash_hasher::IsEnabled for TensorId {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for TensorId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl TensorId {
    #[inline]
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl ArrowField for TensorId {
    type Type = Self;

    #[inline]
    fn data_type() -> arrow2::datatypes::DataType {
        arrow2::datatypes::DataType::FixedSizeBinary(16)
    }
}

//TODO(https://github.com/DataEngineeringLabs/arrow2-convert/issues/79#issue-1415520918)
impl ArrowSerialize for TensorId {
    type MutableArrayType = MutableFixedSizeBinaryArray;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        MutableFixedSizeBinaryArray::new(16)
    }

    #[inline]
    fn arrow_serialize(
        v: &<Self as arrow2_convert::field::ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0.as_bytes()))
    }
}

impl ArrowDeserialize for TensorId {
    type ArrayType = FixedSizeBinaryArray;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v.and_then(|bytes| uuid::Uuid::from_slice(bytes).ok())
            .map(Self)
    }
}

// ----------------------------------------------------------------------------

/// Flattened `Tensor` data payload
///
/// ## Examples
///
/// ```
/// # use re_log_types::component_types::TensorData;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     TensorData::data_type(),
///     DataType::Union(
///         vec![
///             Field::new("U8", DataType::Binary, false),
///             Field::new(
///                 "U16",
///                 DataType::List(Box::new(Field::new("item", DataType::UInt16, false))),
///                 false
///             ),
///             Field::new(
///                 "U32",
///                 DataType::List(Box::new(Field::new("item", DataType::UInt32, false))),
///                 false
///             ),
///             Field::new(
///                 "U64",
///                 DataType::List(Box::new(Field::new("item", DataType::UInt64, false))),
///                 false
///             ),
///             Field::new(
///                 "I8",
///                 DataType::List(Box::new(Field::new("item", DataType::Int8, false))),
///                 false
///             ),
///             Field::new(
///                 "I16",
///                 DataType::List(Box::new(Field::new("item", DataType::Int16, false))),
///                 false
///             ),
///             Field::new(
///                 "I32",
///                 DataType::List(Box::new(Field::new("item", DataType::Int32, false))),
///                 false
///             ),
///             Field::new(
///                 "I64",
///                 DataType::List(Box::new(Field::new("item", DataType::Int64, false))),
///                 false
///             ),
///             Field::new(
///                 "F32",
///                 DataType::List(Box::new(Field::new("item", DataType::Float32, false))),
///                 false
///             ),
///             Field::new(
///                 "F64",
///                 DataType::List(Box::new(Field::new("item", DataType::Float64, false))),
///                 false
///             ),
///             Field::new("JPEG", DataType::Binary, false),
///         ],
///         None,
///         UnionMode::Dense
///     ),
/// );
/// ```
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
pub enum TensorData {
    U8(Vec<u8>),
    U16(Buffer<u16>),
    U32(Buffer<u32>),
    U64(Buffer<u64>),
    // ---
    I8(Buffer<i8>),
    I16(Buffer<i16>),
    I32(Buffer<i32>),
    I64(Buffer<i64>),
    // ---
    // TODO(#854): Native F16 support for arrow tensors
    //F16(Vec<arrow2::types::f16>),
    F32(Buffer<f32>),
    F64(Buffer<f64>),
    JPEG(Vec<u8>),
}

/// Flattened `Tensor` data payload
///
/// ## Examples
///
/// ```
/// # use re_log_types::component_types::TensorDimension;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     TensorDimension::data_type(),
///     DataType::Struct(vec![
///         Field::new("size", DataType::UInt64, false),
///         Field::new("name", DataType::Utf8, true),
///     ])
/// );
/// ```
#[derive(Clone, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
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

    #[inline]
    pub fn height(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_HEIGHT))
    }

    #[inline]
    pub fn width(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_WIDTH))
    }

    #[inline]
    pub fn depth(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_DEPTH))
    }

    #[inline]
    pub fn named(size: u64, name: String) -> Self {
        Self {
            size,
            name: Some(name),
        }
    }
    #[inline]
    pub fn unnamed(size: u64) -> Self {
        Self { size, name: None }
    }
}

impl std::fmt::Debug for TensorDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{}={}", name, self.size)
        } else {
            self.size.fmt(f)
        }
    }
}

impl std::fmt::Display for TensorDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{}={}", name, self.size)
        } else {
            self.size.fmt(f)
        }
    }
}

// TODO(jleibs) This should be extended to include things like rgb vs bgr
#[derive(Clone, Copy, Debug, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum TensorDataMeaning {
    /// Default behavior: guess based on shape
    Unknown,
    /// The data is an annotated [`crate::component_types::ClassId`] which should be
    /// looked up using the appropriate [`crate::context::AnnotationContext`]
    ClassId,
    /// Image data interpreted as depth map.
    Depth,
}

/// A Multi-dimensional Tensor
///
/// ## Examples
///
/// ```
/// # use re_log_types::component_types::{TensorData, TensorDimension, Tensor};
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     Tensor::data_type(),
///     DataType::Struct(vec![
///         Field::new("tensor_id", DataType::FixedSizeBinary(16), false),
///         Field::new(
///             "shape",
///             DataType::List(Box::new(Field::new(
///                 "item",
///                 TensorDimension::data_type(),
///                 false
///             )),),
///             false
///         ),
///         Field::new("data", TensorData::data_type(), false),
///         Field::new(
///             "meaning",
///             DataType::Union(
///                 vec![
///                     Field::new("Unknown", DataType::Boolean, false),
///                     Field::new("ClassId", DataType::Boolean, false),
///                     Field::new("Depth", DataType::Boolean, false)
///                 ],
///                 None,
///                 UnionMode::Dense
///             ),
///             false
///         ),
///         Field::new("meter", DataType::Float32, true),
///     ])
/// );
/// ```
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct Tensor {
    /// Unique identifier for the tensor
    pub tensor_id: TensorId,
    /// Dimensionality and length
    pub shape: Vec<TensorDimension>,
    /// Data payload
    pub data: TensorData,
    /// The per-element data meaning
    /// Used to indicated if the data should be interpreted as color, class_id, etc.
    pub meaning: TensorDataMeaning,
    /// Reciprocal scale of meter unit for depth images
    pub meter: Option<f32>,
}

impl TensorTrait for Tensor {
    #[inline]
    fn id(&self) -> TensorId {
        self.tensor_id
    }

    #[inline]
    fn shape(&self) -> &[TensorDimension] {
        self.shape.as_slice()
    }

    #[inline]
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

    #[inline]
    fn is_vector(&self) -> bool {
        let shape = &self.shape;
        shape.len() == 1 || { shape.len() == 2 && (shape[0].size == 1 || shape[1].size == 1) }
    }

    #[inline]
    fn meaning(&self) -> TensorDataMeaning {
        self.meaning
    }

    fn get(&self, index: &[u64]) -> Option<TensorElement> {
        let mut stride: usize = 1;
        let mut offset: usize = 0;
        for (TensorDimension { size, .. }, index) in self.shape.iter().zip(index).rev() {
            if size <= index {
                return None;
            }
            offset += *index as usize * stride;
            stride *= *size as usize;
        }

        match &self.data {
            TensorData::U8(buf) => Some(TensorElement::U8(buf[offset])),
            TensorData::U16(buf) => Some(TensorElement::U16(buf[offset])),
            TensorData::U32(buf) => Some(TensorElement::U32(buf[offset])),
            TensorData::U64(buf) => Some(TensorElement::U64(buf[offset])),
            TensorData::I8(buf) => Some(TensorElement::I8(buf[offset])),
            TensorData::I16(buf) => Some(TensorElement::I16(buf[offset])),
            TensorData::I32(buf) => Some(TensorElement::I32(buf[offset])),
            TensorData::I64(buf) => Some(TensorElement::I64(buf[offset])),
            TensorData::F32(buf) => Some(TensorElement::F32(buf[offset])),
            TensorData::F64(buf) => Some(TensorElement::F64(buf[offset])),
            TensorData::JPEG(_) => None, // Too expensive to unpack here.
        }
    }
}

impl Component for Tensor {
    #[inline]
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
            TensorData::JPEG(data) => (
                crate::TensorDataType::U8,
                TensorDataStore::Jpeg(Arc::from(data.as_slice())),
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

        impl<'a, D: ::ndarray::Dimension> TryFrom<::ndarray::ArrayView<'a, $type, D>> for Tensor {
            type Error = TensorCastError;

            fn try_from(view: ::ndarray::ArrayView<'a, $type, D>) -> Result<Self, Self::Error> {
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
                        data: TensorData::$variant(Vec::from(slice).into()),
                        meaning: TensorDataMeaning::Unknown,
                        meter: None,
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
                        data: TensorData::$variant(value.into_raw_vec().into()),
                        meaning: TensorDataMeaning::Unknown,
                        meter: None,
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
    dbg!(a0); // NOLINT

    let a = ndarray::Array3::<f64>::zeros((1, 2, 3));
    let t1 = Tensor::try_from(a.into_dyn().view());
    dbg!(t1); // NOLINT
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
            data: TensorData::U16(vec![1, 2, 3, 4].into()),
            meaning: TensorDataMeaning::Unknown,
            meter: Some(1000.0),
        },
        Tensor {
            tensor_id: TensorId(std::default::Default::default()),
            shape: vec![TensorDimension {
                size: 2,
                name: None,
            }],
            data: TensorData::F32(vec![1.23, 2.45].into()),
            meaning: TensorDataMeaning::Unknown,
            meter: None,
        },
    ];

    let array: Box<dyn arrow2::array::Array> = tensors_in.iter().try_into_arrow().unwrap();
    let tensors_out: Vec<Tensor> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(tensors_in, tensors_out);
}

#[test]
fn test_concat_and_slice() {
    use crate::msg_bundle::wrap_in_listarray;
    use arrow2::array::ListArray;
    use arrow2::compute::concatenate::concatenate;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let tensor1 = vec![Tensor {
        tensor_id: TensorId::random(),
        shape: vec![TensorDimension {
            size: 4,
            name: None,
        }],
        data: TensorData::JPEG(vec![1, 2, 3, 4]),
        meaning: TensorDataMeaning::Unknown,
        meter: Some(1000.0),
    }];

    let tensor2 = vec![Tensor {
        tensor_id: TensorId::random(),
        shape: vec![TensorDimension {
            size: 4,
            name: None,
        }],
        data: TensorData::JPEG(vec![5, 6, 7, 8]),
        meaning: TensorDataMeaning::Unknown,
        meter: None,
    }];

    let array1: Box<dyn arrow2::array::Array> = tensor1.iter().try_into_arrow().unwrap();
    let list1 = wrap_in_listarray(array1).boxed();
    let array2: Box<dyn arrow2::array::Array> = tensor2.iter().try_into_arrow().unwrap();
    let list2 = wrap_in_listarray(array2).boxed();

    let pre_concat = list1
        .as_any()
        .downcast_ref::<ListArray<i32>>()
        .unwrap()
        .value(0);

    let tensor_out: Vec<Tensor> = TryIntoCollection::try_into_collection(pre_concat).unwrap();

    assert_eq!(tensor1, tensor_out);

    let concat = concatenate(&[list1.as_ref(), list2.as_ref()]).unwrap();

    let slice = concat
        .as_any()
        .downcast_ref::<ListArray<i32>>()
        .unwrap()
        .value(1);

    let tensor_out: Vec<Tensor> = TryIntoCollection::try_into_collection(slice).unwrap();

    assert_eq!(tensor2[0], tensor_out[0]);
}
