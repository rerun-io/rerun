use arrow2::array::{FixedSizeBinaryArray, MutableFixedSizeBinaryArray};
use arrow2::buffer::Buffer;
use arrow2_convert::deserialize::ArrowDeserialize;
use arrow2_convert::field::ArrowField;
use arrow2_convert::{serialize::ArrowSerialize, ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::{TensorDataType, TensorElement};

// ----------------------------------------------------------------------------

/// A unique id per [`Tensor`].
///
/// TODO(emilk): this should be a hash of the tensor (CAS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TensorId(pub uuid::Uuid);

impl nohash_hasher::IsEnabled for TensorId {}

// required for [`nohash_hasher`].
#[allow(clippy::derived_hash_with_manual_eq)]
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
/// # use re_components::TensorData;
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
///                 "F16",
///                 DataType::List(Box::new(Field::new("item", DataType::Float16, false))),
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
#[derive(Clone, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
#[allow(clippy::upper_case_acronyms)] // TODO(emilk): Rename to `Jpeg`.
pub enum TensorData {
    U8(Buffer<u8>),
    U16(Buffer<u16>),
    U32(Buffer<u32>),
    U64(Buffer<u64>),
    // ---
    I8(Buffer<i8>),
    I16(Buffer<i16>),
    I32(Buffer<i32>),
    I64(Buffer<i64>),
    // ---
    F16(Buffer<arrow2::types::f16>),
    F32(Buffer<f32>),
    F64(Buffer<f64>),
    JPEG(Buffer<u8>),
}

impl TensorData {
    pub fn dtype(&self) -> TensorDataType {
        match self {
            Self::U8(_) | Self::JPEG(_) => TensorDataType::U8,
            Self::U16(_) => TensorDataType::U16,
            Self::U32(_) => TensorDataType::U32,
            Self::U64(_) => TensorDataType::U64,
            Self::I8(_) => TensorDataType::I8,
            Self::I16(_) => TensorDataType::I16,
            Self::I32(_) => TensorDataType::I32,
            Self::I64(_) => TensorDataType::I64,
            Self::F16(_) => TensorDataType::F16,
            Self::F32(_) => TensorDataType::F32,
            Self::F64(_) => TensorDataType::F64,
        }
    }

    pub fn size_in_bytes(&self) -> usize {
        match self {
            Self::U8(buf) | Self::JPEG(buf) => buf.len(),
            Self::U16(buf) => buf.len(),
            Self::U32(buf) => buf.len(),
            Self::U64(buf) => buf.len(),
            Self::I8(buf) => buf.len(),
            Self::I16(buf) => buf.len(),
            Self::I32(buf) => buf.len(),
            Self::I64(buf) => buf.len(),
            Self::F16(buf) => buf.len(),
            Self::F32(buf) => buf.len(),
            Self::F64(buf) => buf.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.size_in_bytes() == 0
    }

    pub fn is_compressed_image(&self) -> bool {
        match self {
            Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::F16(_)
            | Self::F32(_)
            | Self::F64(_) => false,

            Self::JPEG(_) => true,
        }
    }
}

impl std::fmt::Debug for TensorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::U8(_) => write!(f, "U8({} bytes)", self.size_in_bytes()),
            Self::U16(_) => write!(f, "U16({} bytes)", self.size_in_bytes()),
            Self::U32(_) => write!(f, "U32({} bytes)", self.size_in_bytes()),
            Self::U64(_) => write!(f, "U64({} bytes)", self.size_in_bytes()),
            Self::I8(_) => write!(f, "I8({} bytes)", self.size_in_bytes()),
            Self::I16(_) => write!(f, "I16({} bytes)", self.size_in_bytes()),
            Self::I32(_) => write!(f, "I32({} bytes)", self.size_in_bytes()),
            Self::I64(_) => write!(f, "I64({} bytes)", self.size_in_bytes()),
            Self::F16(_) => write!(f, "F16({} bytes)", self.size_in_bytes()),
            Self::F32(_) => write!(f, "F32({} bytes)", self.size_in_bytes()),
            Self::F64(_) => write!(f, "F64({} bytes)", self.size_in_bytes()),
            Self::JPEG(_) => write!(f, "JPEG({} bytes)", self.size_in_bytes()),
        }
    }
}

/// Flattened `Tensor` data payload
///
/// ## Examples
///
/// ```
/// # use re_components::TensorDimension;
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

/// How to interpret the contents of a tensor.
// TODO(jleibs) This should be extended to include things like rgb vs bgr
#[derive(Clone, Copy, Debug, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(type = "dense")]
pub enum TensorDataMeaning {
    /// Default behavior: guess based on shape
    Unknown,

    /// The data is an annotated [`crate::ClassId`] which should be
    /// looked up using the appropriate [`crate::AnnotationContext`]
    ClassId,

    /// Image data interpreted as depth map.
    Depth,
}

/// A Multi-dimensional Tensor.
///
/// All clones are shallow.
///
/// The `Tensor` component is special, as you can only have one instance of it per entity.
/// This is because each element in a tensor is considered to be a separate instance.
///
/// ## Examples
///
/// ```
/// # use re_components::{TensorData, TensorDimension, Tensor};
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

impl Tensor {
    #[inline]
    pub fn id(&self) -> TensorId {
        self.tensor_id
    }

    #[inline]
    pub fn shape(&self) -> &[TensorDimension] {
        self.shape.as_slice()
    }

    /// Returns the shape of the tensor with all trailing dimensions of size 1 ignored.
    ///
    /// If all dimension sizes are one, this returns only the first dimension.
    #[inline]
    pub fn shape_short(&self) -> &[TensorDimension] {
        if self.shape.is_empty() {
            &self.shape
        } else {
            self.shape
                .iter()
                .enumerate()
                .rev()
                .find(|(_, dim)| dim.size != 1)
                .map_or(&self.shape[0..1], |(i, _)| &self.shape[..(i + 1)])
        }
    }

    #[inline]
    pub fn num_dim(&self) -> usize {
        self.shape.len()
    }

    /// If the tensor can be interpreted as an image, return the height, width, and channels/depth of it.
    pub fn image_height_width_channels(&self) -> Option<[u64; 3]> {
        let shape_short = self.shape_short();

        match shape_short.len() {
            1 => {
                // Special case: Nx1(x1x1x…) tensors are treated as Nx1 gray images.
                // Special case: Nx1(x1x1x…) tensors are treated as Nx1 gray images.
                if self.shape.len() >= 2 {
                    Some([shape_short[0].size, 1, 1])
                } else {
                    None
                }
            }
            2 => Some([shape_short[0].size, shape_short[1].size, 1]),
            3 => {
                let channels = shape_short[2].size;
                if matches!(channels, 3 | 4) {
                    // rgb, rgba
                    Some([shape_short[0].size, shape_short[1].size, channels])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns true if the tensor can be interpreted as an image.
    pub fn is_shaped_like_an_image(&self) -> bool {
        self.image_height_width_channels().is_some()
    }

    /// Returns true if either all dimensions have size 1 or only a single dimension has a size larger than 1.
    ///
    /// Empty tensors return false.
    #[inline]
    pub fn is_vector(&self) -> bool {
        if self.shape.is_empty() {
            false
        } else {
            self.shape.iter().filter(|dim| dim.size > 1).count() <= 1
        }
    }

    #[inline]
    pub fn meaning(&self) -> TensorDataMeaning {
        self.meaning
    }

    /// Query with x, y, channel indices.
    ///
    /// Allows to query values for any image like tensor even if it has more or less dimensions than 3.
    /// (useful for sampling e.g. `N x M x C x 1` tensor which is a valid image)
    #[inline]
    pub fn get_with_image_coords(&self, x: u64, y: u64, channel: u64) -> Option<TensorElement> {
        match self.shape.len() {
            1 => {
                if y == 0 && channel == 0 {
                    self.get(&[x])
                } else {
                    None
                }
            }
            2 => {
                if channel == 0 {
                    self.get(&[y, x])
                } else {
                    None
                }
            }
            3 => self.get(&[y, x, channel]),
            4 => {
                // Optimization for common case, next case handles this too.
                if self.shape[3].size == 1 {
                    self.get(&[y, x, channel, 0])
                } else {
                    None
                }
            }
            dim => self.image_height_width_channels().and_then(|_| {
                self.get(
                    &[x, y, channel]
                        .into_iter()
                        .chain(std::iter::repeat(0).take(dim - 3))
                        .collect::<Vec<u64>>(),
                )
            }),
        }
    }

    pub fn get(&self, index: &[u64]) -> Option<TensorElement> {
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
            TensorData::F16(buf) => Some(TensorElement::F16(buf[offset])),
            TensorData::F32(buf) => Some(TensorElement::F32(buf[offset])),
            TensorData::F64(buf) => Some(TensorElement::F64(buf[offset])),
            TensorData::JPEG(_) => None, // Too expensive to unpack here.
        }
    }

    pub fn dtype(&self) -> TensorDataType {
        self.data.dtype()
    }

    pub fn size_in_bytes(&self) -> usize {
        self.data.size_in_bytes()
    }
}

impl re_log_types::LegacyComponent for Tensor {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.tensor".into()
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Clone)]
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

                match view.to_slice() {
                    Some(slice) => Ok(Tensor {
                        tensor_id: TensorId::random(),
                        shape,
                        data: TensorData::$variant(Vec::from(slice).into()),
                        meaning: TensorDataMeaning::Unknown,
                        meter: None,
                    }),
                    None => Ok(Tensor {
                        tensor_id: TensorId::random(),
                        shape,
                        data: TensorData::$variant(view.iter().cloned().collect::<Vec<_>>().into()),
                        meaning: TensorDataMeaning::Unknown,
                        meter: None,
                    }),
                }
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

tensor_type!(arrow2::types::f16, F16);
tensor_type!(f32, F32);
tensor_type!(f64, F64);

// Manual expansion of tensor_type! macro for `half::f16` types. We need to do this
// because arrow uses its own half type. The two use the same underlying representation
// but are still distinct types. `half::f16`, however, is more full-featured and
// generally a better choice to use when converting to ndarray.
// ==========================================
// TODO(jleibs): would be nice to support this with the macro definition as well
// but the bytemuck casts add a bit of complexity here.
impl<'a> TryFrom<&'a Tensor> for ::ndarray::ArrayViewD<'a, half::f16> {
    type Error = TensorCastError;

    fn try_from(value: &'a Tensor) -> Result<Self, Self::Error> {
        let shape: Vec<_> = value.shape.iter().map(|d| d.size as usize).collect();
        if let TensorData::F16(data) = &value.data {
            ndarray::ArrayViewD::from_shape(shape, bytemuck::cast_slice(data.as_slice()))
                .map_err(|err| TensorCastError::BadTensorShape { source: err })
        } else {
            Err(TensorCastError::TypeMismatch)
        }
    }
}

impl<'a, D: ::ndarray::Dimension> TryFrom<::ndarray::ArrayView<'a, half::f16, D>> for Tensor {
    type Error = TensorCastError;

    fn try_from(view: ::ndarray::ArrayView<'a, half::f16, D>) -> Result<Self, Self::Error> {
        let shape = view
            .shape()
            .iter()
            .map(|dim| TensorDimension {
                size: *dim as u64,
                name: None,
            })
            .collect();
        match view.to_slice() {
            Some(slice) => Ok(Tensor {
                tensor_id: TensorId::random(),
                shape,
                data: TensorData::F16(Vec::from(bytemuck::cast_slice(slice)).into()),
                meaning: TensorDataMeaning::Unknown,
                meter: None,
            }),
            None => Ok(Tensor {
                tensor_id: TensorId::random(),
                shape,
                data: TensorData::F16(
                    view.iter()
                        .map(|f| arrow2::types::f16::from_bits(f.to_bits()))
                        .collect::<Vec<_>>()
                        .into(),
                ),
                meaning: TensorDataMeaning::Unknown,
                meter: None,
            }),
        }
    }
}

impl<D: ::ndarray::Dimension> TryFrom<::ndarray::Array<half::f16, D>> for Tensor {
    type Error = TensorCastError;

    fn try_from(value: ndarray::Array<half::f16, D>) -> Result<Self, Self::Error> {
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
                data: TensorData::F16(
                    bytemuck::cast_slice(value.into_raw_vec().as_slice())
                        .to_vec()
                        .into(),
                ),
                meaning: TensorDataMeaning::Unknown,
                meter: None,
            })
            .ok_or(TensorCastError::NotContiguousStdOrder)
    }
}

// ----------------------------------------------------------------------------

/// Errors when loading [`Tensor`] from the [`image`] crate.
#[cfg(feature = "image")]
#[derive(thiserror::Error, Clone, Debug)]
pub enum TensorImageLoadError {
    #[error(transparent)]
    Image(std::sync::Arc<image::ImageError>),

    #[error("Expected a HxW, HxWx1 or HxWx3 tensor, but got {0:?}")]
    UnexpectedJpegShape(Vec<TensorDimension>),

    #[error("Unsupported color type: {0:?}. We support 8-bit, 16-bit, and f32 images, and RGB, RGBA, Luminance, and Luminance-Alpha.")]
    UnsupportedImageColorType(image::ColorType),

    #[error("Failed to load file: {0}")]
    ReadError(std::sync::Arc<std::io::Error>),

    #[error("The encoded tensor shape did not match its metadata {expected:?} != {found:?}")]
    InvalidMetaData { expected: Vec<u64>, found: Vec<u64> },

    #[error(transparent)]
    JpegDecode(#[from] zune_jpeg::errors::DecodeErrors),
}

#[cfg(feature = "image")]
impl From<image::ImageError> for TensorImageLoadError {
    #[inline]
    fn from(err: image::ImageError) -> Self {
        TensorImageLoadError::Image(std::sync::Arc::new(err))
    }
}

#[cfg(feature = "image")]
impl From<std::io::Error> for TensorImageLoadError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        TensorImageLoadError::ReadError(std::sync::Arc::new(err))
    }
}

/// Errors when converting [`Tensor`] to [`image`] images.
#[cfg(feature = "image")]
#[derive(thiserror::Error, Debug)]
pub enum TensorImageSaveError {
    #[error("Expected image-shaped tensor, got {0:?}")]
    ShapeNotAnImage(Vec<TensorDimension>),

    #[error("Cannot convert tensor with {0} channels and datatype {1} to an image")]
    UnsupportedChannelsDtype(u64, TensorDataType),

    #[error("The tensor data did not match tensor dimensions")]
    BadData,
}

impl Tensor {
    pub fn new(
        tensor_id: TensorId,
        shape: Vec<TensorDimension>,
        data: TensorData,
        meaning: TensorDataMeaning,
        meter: Option<f32>,
    ) -> Self {
        Self {
            tensor_id,
            shape,
            data,
            meaning,
            meter,
        }
    }
}

#[cfg(feature = "image")]
impl Tensor {
    /// Construct a tensor from the contents of an image file on disk.
    ///
    /// JPEGs will be kept encoded, left to the viewer to decode on-the-fly.
    /// Other images types will be decoded directly.
    ///
    /// Requires the `image` feature.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_image_file(path: &std::path::Path) -> Result<Self, TensorImageLoadError> {
        re_tracing::profile_function!(path.to_string_lossy());

        let img_bytes = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(path)?
        };

        let img_format = if let Some(extension) = path.extension() {
            if let Some(format) = image::ImageFormat::from_extension(extension) {
                format
            } else {
                image::guess_format(&img_bytes)?
            }
        } else {
            image::guess_format(&img_bytes)?
        };

        Self::from_image_bytes(img_bytes, img_format)
    }

    /// Construct a tensor from the contents of a JPEG file on disk.
    ///
    /// Requires the `image` feature.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_jpeg_file(path: &std::path::Path) -> Result<Self, TensorImageLoadError> {
        re_tracing::profile_function!(path.to_string_lossy());
        let jpeg_bytes = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(path)?
        };
        Self::from_jpeg_bytes(jpeg_bytes)
    }

    #[deprecated = "Renamed 'from_jpeg_file'"]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tensor_from_jpeg_file(
        image_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, TensorImageLoadError> {
        Self::from_jpeg_file(image_path.as_ref())
    }

    /// Construct a tensor from the contents of an image file.
    ///
    /// JPEGs will be kept encoded, left to the viewer to decode on-the-fly.
    /// Other images types will be decoded directly.
    ///
    /// Requires the `image` feature.
    pub fn from_image_bytes(
        bytes: Vec<u8>,
        format: image::ImageFormat,
    ) -> Result<Self, TensorImageLoadError> {
        re_tracing::profile_function!(format!("{format:?}"));
        if format == image::ImageFormat::Jpeg {
            Self::from_jpeg_bytes(bytes)
        } else {
            let image = image::load_from_memory_with_format(&bytes, format)?;
            Self::from_image(image)
        }
    }

    /// Construct a tensor from the contents of a JPEG file, without decoding it now.
    ///
    /// Requires the `image` feature.
    pub fn from_jpeg_bytes(jpeg_bytes: Vec<u8>) -> Result<Self, TensorImageLoadError> {
        re_tracing::profile_function!();

        use zune_jpeg::JpegDecoder;

        let mut decoder = JpegDecoder::new(&jpeg_bytes);
        decoder.decode_headers()?;
        let (w, h) = decoder.dimensions().unwrap(); // Can't fail after a successful decode_headers

        Ok(Self {
            tensor_id: TensorId::random(),
            shape: vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
                TensorDimension::depth(3),
            ],
            data: TensorData::JPEG(jpeg_bytes.into()),
            meaning: TensorDataMeaning::Unknown,
            meter: None,
        })
    }

    #[deprecated = "Renamed 'from_jpeg_bytes'"]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tensor_from_jpeg_bytes(jpeg_bytes: Vec<u8>) -> Result<Self, TensorImageLoadError> {
        Self::from_jpeg_bytes(jpeg_bytes)
    }

    /// Construct a tensor from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    ///
    /// This is a convenience function that calls [`DecodedTensor::from_image`].
    pub fn from_image(
        image: impl Into<image::DynamicImage>,
    ) -> Result<Tensor, TensorImageLoadError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    ///
    /// This is a convenience function that calls [`DecodedTensor::from_dynamic_image`].
    pub fn from_dynamic_image(image: image::DynamicImage) -> Result<Tensor, TensorImageLoadError> {
        DecodedTensor::from_dynamic_image(image).map(DecodedTensor::into_inner)
    }

    /// Predicts if [`Self::to_dynamic_image`] is likely to succeed, without doing anything expensive
    pub fn could_be_dynamic_image(&self) -> bool {
        self.is_shaped_like_an_image()
            && matches!(
                self.dtype(),
                TensorDataType::U8
                    | TensorDataType::U16
                    | TensorDataType::F16
                    | TensorDataType::F32
                    | TensorDataType::F64
            )
    }

    /// Try to convert an image-like tensor into an [`image::DynamicImage`].
    pub fn to_dynamic_image(&self) -> Result<image::DynamicImage, TensorImageSaveError> {
        use ecolor::{gamma_u8_from_linear_f32, linear_u8_from_linear_f32};
        use image::{DynamicImage, GrayImage, RgbImage, RgbaImage};

        type Rgb16Image = image::ImageBuffer<image::Rgb<u16>, Vec<u16>>;
        type Rgba16Image = image::ImageBuffer<image::Rgba<u16>, Vec<u16>>;
        type Gray16Image = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;

        let [h, w, channels] = self
            .image_height_width_channels()
            .ok_or_else(|| TensorImageSaveError::ShapeNotAnImage(self.shape.clone()))?;
        let w = w as u32;
        let h = h as u32;

        let dyn_img_result =
            match (channels, &self.data) {
                (1, TensorData::U8(buf)) => {
                    GrayImage::from_raw(w, h, buf.as_slice().to_vec()).map(DynamicImage::ImageLuma8)
                }
                (1, TensorData::U16(buf)) => Gray16Image::from_raw(w, h, buf.as_slice().to_vec())
                    .map(DynamicImage::ImageLuma16),
                // TODO(emilk) f16
                (1, TensorData::F32(buf)) => {
                    let pixels = buf
                        .iter()
                        .map(|pixel| gamma_u8_from_linear_f32(*pixel))
                        .collect();
                    GrayImage::from_raw(w, h, pixels).map(DynamicImage::ImageLuma8)
                }
                (1, TensorData::F64(buf)) => {
                    let pixels = buf
                        .iter()
                        .map(|&pixel| gamma_u8_from_linear_f32(pixel as f32))
                        .collect();
                    GrayImage::from_raw(w, h, pixels).map(DynamicImage::ImageLuma8)
                }

                (3, TensorData::U8(buf)) => {
                    RgbImage::from_raw(w, h, buf.as_slice().to_vec()).map(DynamicImage::ImageRgb8)
                }
                (3, TensorData::U16(buf)) => Rgb16Image::from_raw(w, h, buf.as_slice().to_vec())
                    .map(DynamicImage::ImageRgb16),
                (3, TensorData::F32(buf)) => {
                    let pixels = buf.iter().copied().map(gamma_u8_from_linear_f32).collect();
                    RgbImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgb8)
                }
                (3, TensorData::F64(buf)) => {
                    let pixels = buf
                        .iter()
                        .map(|&comp| gamma_u8_from_linear_f32(comp as f32))
                        .collect();
                    RgbImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgb8)
                }

                (4, TensorData::U8(buf)) => {
                    RgbaImage::from_raw(w, h, buf.as_slice().to_vec()).map(DynamicImage::ImageRgba8)
                }
                (4, TensorData::U16(buf)) => Rgba16Image::from_raw(w, h, buf.as_slice().to_vec())
                    .map(DynamicImage::ImageRgba16),
                (4, TensorData::F32(buf)) => {
                    let rgba: &[[f32; 4]] = bytemuck::cast_slice(buf.as_slice());
                    let pixels: Vec<u8> = rgba
                        .iter()
                        .flat_map(|&[r, g, b, a]| {
                            let r = gamma_u8_from_linear_f32(r);
                            let g = gamma_u8_from_linear_f32(g);
                            let b = gamma_u8_from_linear_f32(b);
                            let a = linear_u8_from_linear_f32(a);
                            [r, g, b, a]
                        })
                        .collect();
                    RgbaImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgba8)
                }
                (4, TensorData::F64(buf)) => {
                    let rgba: &[[f64; 4]] = bytemuck::cast_slice(buf.as_slice());
                    let pixels: Vec<u8> = rgba
                        .iter()
                        .flat_map(|&[r, g, b, a]| {
                            let r = gamma_u8_from_linear_f32(r as _);
                            let g = gamma_u8_from_linear_f32(g as _);
                            let b = gamma_u8_from_linear_f32(b as _);
                            let a = linear_u8_from_linear_f32(a as _);
                            [r, g, b, a]
                        })
                        .collect();
                    RgbaImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgba8)
                }

                (_, _) => {
                    return Err(TensorImageSaveError::UnsupportedChannelsDtype(
                        channels,
                        self.data.dtype(),
                    ))
                }
            };

        dyn_img_result.ok_or(TensorImageSaveError::BadData)
    }
}

// ----------------------------------------------------------------------------

/// A thin wrapper around a [`Tensor`] that is guaranteed to not be compressed (never a jpeg).
///
/// All clones are shallow, like for [`Tensor`].
#[derive(Clone)]
pub struct DecodedTensor(Tensor);

impl DecodedTensor {
    #[inline(always)]
    pub fn inner(&self) -> &Tensor {
        &self.0
    }

    #[inline(always)]
    pub fn into_inner(self) -> Tensor {
        self.0
    }
}

impl TryFrom<Tensor> for DecodedTensor {
    type Error = Tensor;

    fn try_from(tensor: Tensor) -> Result<Self, Tensor> {
        match &tensor.data {
            TensorData::U8(_)
            | TensorData::U16(_)
            | TensorData::U32(_)
            | TensorData::U64(_)
            | TensorData::I8(_)
            | TensorData::I16(_)
            | TensorData::I32(_)
            | TensorData::I64(_)
            | TensorData::F16(_)
            | TensorData::F32(_)
            | TensorData::F64(_) => Ok(Self(tensor)),

            TensorData::JPEG(_) => Err(tensor),
        }
    }
}

#[cfg(feature = "image")]
impl DecodedTensor {
    /// Construct a tensor from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(
        image: impl Into<image::DynamicImage>,
    ) -> Result<DecodedTensor, TensorImageLoadError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(
        image: image::DynamicImage,
    ) -> Result<DecodedTensor, TensorImageLoadError> {
        re_tracing::profile_function!();

        let (w, h) = (image.width(), image.height());

        let (depth, data) = match image {
            image::DynamicImage::ImageLuma8(image) => (1, TensorData::U8(image.into_raw().into())),
            image::DynamicImage::ImageRgb8(image) => (3, TensorData::U8(image.into_raw().into())),
            image::DynamicImage::ImageRgba8(image) => (4, TensorData::U8(image.into_raw().into())),
            image::DynamicImage::ImageLuma16(image) => {
                (1, TensorData::U16(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgb16(image) => (3, TensorData::U16(image.into_raw().into())),
            image::DynamicImage::ImageRgba16(image) => {
                (4, TensorData::U16(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgb32F(image) => {
                (3, TensorData::F32(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgba32F(image) => {
                (4, TensorData::F32(image.into_raw().into()))
            }
            image::DynamicImage::ImageLumaA8(image) => {
                re_log::warn!(
                    "Rerun doesn't have native support for 8-bit Luma + Alpha. The image will be convert to RGBA."
                );
                return Self::from_image(image::DynamicImage::ImageLumaA8(image).to_rgba8());
            }
            image::DynamicImage::ImageLumaA16(image) => {
                re_log::warn!(
                    "Rerun doesn't have native support for 16-bit Luma + Alpha. The image will be convert to RGBA."
                );
                return Self::from_image(image::DynamicImage::ImageLumaA16(image).to_rgba16());
            }
            _ => {
                // It is very annoying that DynamicImage is #[non_exhaustive]
                return Err(TensorImageLoadError::UnsupportedImageColorType(
                    image.color(),
                ));
            }
        };
        let tensor = Tensor {
            tensor_id: TensorId::random(),
            shape: vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
                TensorDimension::depth(depth),
            ],
            data,
            meaning: TensorDataMeaning::Unknown,
            meter: None,
        };
        Ok(DecodedTensor(tensor))
    }

    pub fn try_decode(maybe_encoded_tensor: Tensor) -> Result<Self, TensorImageLoadError> {
        match &maybe_encoded_tensor.data {
            TensorData::U8(_)
            | TensorData::U16(_)
            | TensorData::U32(_)
            | TensorData::U64(_)
            | TensorData::I8(_)
            | TensorData::I16(_)
            | TensorData::I32(_)
            | TensorData::I64(_)
            | TensorData::F16(_)
            | TensorData::F32(_)
            | TensorData::F64(_) => Ok(Self(maybe_encoded_tensor)),

            TensorData::JPEG(jpeg_bytes) => {
                let [h, w, c] = maybe_encoded_tensor
                    .image_height_width_channels()
                    .ok_or_else(|| {
                        TensorImageLoadError::UnexpectedJpegShape(
                            maybe_encoded_tensor.shape().to_vec(),
                        )
                    })?;

                Self::decode_jpeg_bytes(jpeg_bytes, [h, w, c])
            }
        }
    }

    pub fn decode_jpeg_bytes(
        jpeg_bytes: &Buffer<u8>,
        [expected_height, expected_width, expected_channels]: [u64; 3],
    ) -> Result<DecodedTensor, TensorImageLoadError> {
        re_tracing::profile_function!();

        re_log::debug!("Decoding {expected_width}x{expected_height} JPEG");

        use zune_core::colorspace::ColorSpace;
        use zune_core::options::DecoderOptions;
        use zune_jpeg::JpegDecoder;

        let mut options = DecoderOptions::default();

        let depth = if expected_channels == 1 {
            options = options.jpeg_set_out_colorspace(ColorSpace::Luma);
            1
        } else {
            // We decode to RGBA directly so we don't need to pad to four bytes later when uploading to GPU.
            options = options.jpeg_set_out_colorspace(ColorSpace::RGBA);
            4
        };

        let mut decoder = JpegDecoder::new_with_options(options, jpeg_bytes);
        let pixels = decoder.decode()?;
        let (w, h) = decoder.dimensions().unwrap(); // Can't fail after a successful decode

        let (w, h) = (w as u64, h as u64);

        if w != expected_width || h != expected_height {
            return Err(TensorImageLoadError::InvalidMetaData {
                expected: [expected_height, expected_width, expected_channels].into(),
                found: [h, w, depth].into(),
            });
        }

        if pixels.len() as u64 != w * h * depth {
            return Err(zune_jpeg::errors::DecodeErrors::Format(format!(
                "Bug in zune-jpeg: Expected {w}x{h}x{depth}={} bytes, got {}",
                w * h * depth,
                pixels.len()
            ))
            .into());
        }

        let tensor = Tensor {
            tensor_id: TensorId::random(),
            shape: vec![
                TensorDimension::height(h),
                TensorDimension::width(w),
                TensorDimension::depth(depth),
            ],
            data: TensorData::U8(pixels.into()),
            meaning: TensorDataMeaning::Unknown,
            meter: None,
        };
        let decoded_tensor = DecodedTensor(tensor);

        Ok(decoded_tensor)
    }
}

impl AsRef<Tensor> for DecodedTensor {
    #[inline(always)]
    fn as_ref(&self) -> &Tensor {
        &self.0
    }
}

impl std::ops::Deref for DecodedTensor {
    type Target = Tensor;

    #[inline(always)]
    fn deref(&self) -> &Tensor {
        &self.0
    }
}

impl std::borrow::Borrow<Tensor> for DecodedTensor {
    #[inline(always)]
    fn borrow(&self) -> &Tensor {
        &self.0
    }
}

re_log_types::component_legacy_shim!(Tensor);

// ----------------------------------------------------------------------------

#[cfg(feature = "disabled")]
#[test]
fn test_ndarray() {
    let t0 = Tensor {
        tensor_id: TensorId::random(),
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
        data: TensorData::U16(vec![1, 2, 3, 4].into()),
        meaning: TensorDataMeaning::Unknown,
        meter: None,
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
fn test_tensor_shape_utilities() {
    fn generate_tensor_from_shape(sizes: &[u64]) -> Tensor {
        let shape = sizes
            .iter()
            .map(|&size| TensorDimension { size, name: None })
            .collect();
        let num_elements = sizes.iter().fold(0, |acc, &size| acc * size);
        let data = (0..num_elements).map(|i| i as u32).collect::<Vec<_>>();

        Tensor {
            tensor_id: TensorId(std::default::Default::default()),
            shape,
            data: TensorData::U32(data.into()),
            meaning: TensorDataMeaning::Unknown,
            meter: None,
        }
    }

    // Empty tensor.
    {
        let tensor = generate_tensor_from_shape(&[]);

        assert_eq!(tensor.image_height_width_channels(), None);
        assert_eq!(tensor.shape_short(), tensor.shape());
        assert!(!tensor.is_vector());
        assert!(!tensor.is_shaped_like_an_image());
    }

    // Single dimension tensors.
    for shape in [vec![4], vec![1]] {
        let tensor = generate_tensor_from_shape(&shape);

        assert_eq!(tensor.image_height_width_channels(), None);
        assert_eq!(tensor.shape_short(), &tensor.shape()[0..1]);
        assert!(tensor.is_vector());
        assert!(!tensor.is_shaped_like_an_image());
    }

    // Single element, but it might be interpreted as a 1x1 gray image!
    for shape in [
        vec![1, 1],
        vec![1, 1, 1],
        vec![1, 1, 1, 1],
        vec![1, 1, 1, 1, 1],
    ] {
        let tensor = generate_tensor_from_shape(&shape);

        assert_eq!(tensor.image_height_width_channels(), Some([1, 1, 1]));
        assert_eq!(tensor.shape_short(), &tensor.shape()[0..1]);
        assert!(tensor.is_vector());
        assert!(tensor.is_shaped_like_an_image());
    }
    // Color/Gray 2x4 images
    for shape in [
        vec![4, 2],
        vec![4, 2, 1],
        vec![4, 2, 1, 1],
        vec![4, 2, 3],
        vec![4, 2, 3, 1, 1],
        vec![4, 2, 4],
        vec![4, 2, 4, 1, 1, 1, 1],
    ] {
        let tensor = generate_tensor_from_shape(&shape);
        let channels = shape.get(2).cloned().unwrap_or(1);

        assert_eq!(tensor.image_height_width_channels(), Some([4, 2, channels]));
        assert_eq!(
            tensor.shape_short(),
            &tensor.shape()[0..(2 + (channels != 1) as usize)]
        );
        assert!(!tensor.is_vector());
        assert!(tensor.is_shaped_like_an_image());
    }

    // Gray 1x4 images
    for shape in [
        vec![4, 1],
        vec![4, 1, 1],
        vec![4, 1, 1, 1],
        vec![4, 1, 1, 1, 1],
    ] {
        let tensor = generate_tensor_from_shape(&shape);

        assert_eq!(tensor.image_height_width_channels(), Some([4, 1, 1]));
        assert_eq!(tensor.shape_short(), &tensor.shape()[0..1]);
        assert!(tensor.is_vector());
        assert!(tensor.is_shaped_like_an_image());
    }

    // Gray 4x1 images
    for shape in [
        vec![1, 4],
        vec![1, 4, 1],
        vec![1, 4, 1, 1],
        vec![1, 4, 1, 1, 1],
    ] {
        let tensor = generate_tensor_from_shape(&shape);

        assert_eq!(tensor.image_height_width_channels(), Some([1, 4, 1]));
        assert_eq!(tensor.shape_short(), &tensor.shape()[0..2]);
        assert!(tensor.is_vector());
        assert!(tensor.is_shaped_like_an_image());
    }

    // Non images & non vectors without trailing dimensions
    for shape in [vec![4, 2, 5], vec![1, 1, 1, 2, 4]] {
        let tensor = generate_tensor_from_shape(&shape);

        assert_eq!(tensor.image_height_width_channels(), None);
        assert_eq!(tensor.shape_short(), tensor.shape());
        assert!(!tensor.is_vector());
        assert!(!tensor.is_shaped_like_an_image());
    }
}
