use std::sync::Arc;

use half::f16;

use crate::field_types;

pub use crate::field_types::{Arrow3D, Pinhole, Rigid3, Transform};

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BBox2D {
    /// Upper left corner.
    pub min: [f32; 2],
    /// Lower right corner.
    pub max: [f32; 2],
}

/// Oriented 3D box
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Box3 {
    pub rotation: Quaternion,
    pub translation: [f32; 3],
    pub half_size: [f32; 3],
}

/// Order: XYZW
pub type Quaternion = [f32; 4];

// ----------------------------------------------------------------------------

/// The data types supported by a [`ClassicTensor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TensorDataType {
    /// Unsigned 8 bit integer.
    ///
    /// Commonly used for sRGB(A).
    U8,

    /// Unsigned 16 bit integer.
    ///
    /// Used by some depth images and some high-bitrate images.
    U16,

    /// Unsigned 32 bit integer.
    U32,

    /// Unsigned 64 bit integer.
    U64,

    /// Signed 8 bit integer.
    I8,

    /// Signed 16 bit integer.
    I16,

    /// Signed 32 bit integer.
    I32,

    /// Signed 64 bit integer.
    I64,

    /// 16-bit floating point number.
    ///
    /// Uses the standard IEEE 754-2008 binary16 format.
    /// Set <https://en.wikipedia.org/wiki/Half-precision_floating-point_format>.
    F16,

    /// 32-bit floating point number.
    F32,

    /// 64-bit floating point number.
    F64,
}

impl TensorDataType {
    /// Number of bytes used by the type
    #[inline]
    pub fn size(&self) -> u64 {
        match self {
            Self::U8 => std::mem::size_of::<u8>() as _,
            Self::U16 => std::mem::size_of::<u16>() as _,
            Self::U32 => std::mem::size_of::<u32>() as _,
            Self::U64 => std::mem::size_of::<u64>() as _,

            Self::I8 => std::mem::size_of::<i8>() as _,
            Self::I16 => std::mem::size_of::<i16>() as _,
            Self::I32 => std::mem::size_of::<i32>() as _,
            Self::I64 => std::mem::size_of::<i64>() as _,

            Self::F16 => std::mem::size_of::<f16>() as _,
            Self::F32 => std::mem::size_of::<f32>() as _,
            Self::F64 => std::mem::size_of::<f64>() as _,
        }
    }
}

impl std::fmt::Display for TensorDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::U8 => "uint8".fmt(f),
            Self::U16 => "uint16".fmt(f),
            Self::U32 => "uint32".fmt(f),
            Self::U64 => "uint64".fmt(f),

            Self::I8 => "int8".fmt(f),
            Self::I16 => "int16".fmt(f),
            Self::I32 => "int32".fmt(f),
            Self::I64 => "int64".fmt(f),

            Self::F16 => "float16".fmt(f),
            Self::F32 => "float32".fmt(f),
            Self::F64 => "float64".fmt(f),
        }
    }
}

// ----------------------------------------------------------------------------

pub trait TensorDataTypeTrait: Copy + Clone + Send + Sync {
    const DTYPE: TensorDataType;
}

impl TensorDataTypeTrait for u8 {
    const DTYPE: TensorDataType = TensorDataType::U8;
}
impl TensorDataTypeTrait for u16 {
    const DTYPE: TensorDataType = TensorDataType::U16;
}
impl TensorDataTypeTrait for u32 {
    const DTYPE: TensorDataType = TensorDataType::U32;
}
impl TensorDataTypeTrait for u64 {
    const DTYPE: TensorDataType = TensorDataType::U64;
}
impl TensorDataTypeTrait for i8 {
    const DTYPE: TensorDataType = TensorDataType::I8;
}
impl TensorDataTypeTrait for i16 {
    const DTYPE: TensorDataType = TensorDataType::I16;
}
impl TensorDataTypeTrait for i32 {
    const DTYPE: TensorDataType = TensorDataType::I32;
}
impl TensorDataTypeTrait for i64 {
    const DTYPE: TensorDataType = TensorDataType::I64;
}
impl TensorDataTypeTrait for f16 {
    const DTYPE: TensorDataType = TensorDataType::F16;
}
impl TensorDataTypeTrait for f32 {
    const DTYPE: TensorDataType = TensorDataType::F32;
}
impl TensorDataTypeTrait for f64 {
    const DTYPE: TensorDataType = TensorDataType::F64;
}

/// The data that can be stored in a [`ClassicTensor`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TensorElement {
    /// Unsigned 8 bit integer.
    ///
    /// Commonly used for sRGB(A).
    U8(u8),

    /// Unsigned 16 bit integer.
    ///
    /// Used by some depth images and some high-bitrate images.
    U16(u16),

    /// Unsigned 32 bit integer.
    U32(u32),

    /// Unsigned 64 bit integer.
    U64(u64),

    /// Signed 8 bit integer.
    I8(i8),

    /// Signed 16 bit integer.
    I16(i16),

    /// Signed 32 bit integer.
    I32(i32),

    /// Signed 64 bit integer.
    I64(i64),

    /// 16-bit floating point number.
    ///
    /// Uses the standard IEEE 754-2008 binary16 format.
    /// Set <https://en.wikipedia.org/wiki/Half-precision_floating-point_format>.
    F16(f16),

    /// 32-bit floating point number.
    F32(f32),

    /// 64-bit floating point number.
    F64(f64),
}

impl TensorElement {
    #[inline]
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::U8(value) => *value as _,
            Self::U16(value) => *value as _,
            Self::U32(value) => *value as _,
            Self::U64(value) => *value as _,

            Self::I8(value) => *value as _,
            Self::I16(value) => *value as _,
            Self::I32(value) => *value as _,
            Self::I64(value) => *value as _,

            Self::F16(value) => value.to_f64(),
            Self::F32(value) => *value as _,
            Self::F64(value) => *value,
        }
    }

    #[inline]
    pub fn try_as_u16(&self) -> Option<u16> {
        fn u16_from_f64(f: f64) -> Option<u16> {
            let u16_value = f as u16;
            let roundtrips = u16_value as f64 == f;
            roundtrips.then_some(u16_value)
        }

        match self {
            Self::U8(value) => Some(*value as u16),
            Self::U16(value) => Some(*value),
            Self::U32(value) => u16::try_from(*value).ok(),
            Self::U64(value) => u16::try_from(*value).ok(),

            Self::I8(value) => u16::try_from(*value).ok(),
            Self::I16(value) => u16::try_from(*value).ok(),
            Self::I32(value) => u16::try_from(*value).ok(),
            Self::I64(value) => u16::try_from(*value).ok(),

            Self::F16(value) => u16_from_f64(value.to_f64()),
            Self::F32(value) => u16_from_f64(*value as f64),
            Self::F64(value) => u16_from_f64(*value),
        }
    }
}

/// The data types supported by a [`ClassicTensor`].
///
/// NOTE: `PartialEq` takes into account _how_ the data is stored,
/// which can be surprising! As of 2022-08-15, `PartialEq` is only used by tests.
///
/// [`TensorDataStore`] uses [`Arc`] internally so that cloning a [`ClassicTensor`] is cheap
/// and memory efficient.
/// This is crucial, since we clone data for different timelines in the data store.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TensorDataStore {
    /// Densely packed tensor
    Dense(Arc<[u8]>),

    /// A JPEG image.
    ///
    /// This can only represent tensors with [`TensorDataType::U8`]
    /// of dimensions `[h, w, 3]` (RGB) or `[h, w]` (grayscale).
    Jpeg(Arc<[u8]>),
}

impl TensorDataStore {
    pub fn as_slice<T: bytemuck::Pod>(&self) -> Option<&[T]> {
        match self {
            TensorDataStore::Dense(bytes) => Some(bytemuck::cast_slice(bytes)),
            TensorDataStore::Jpeg(_) => None,
        }
    }
}

impl std::fmt::Debug for TensorDataStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TensorDataStore::Dense(bytes) => {
                f.write_fmt(format_args!("TensorData::Dense({} bytes)", bytes.len()))
            }
            TensorDataStore::Jpeg(bytes) => {
                f.write_fmt(format_args!("TensorData::Jpeg({} bytes)", bytes.len()))
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// An N-dimensional collection of numbers.
///
/// Most often used to describe image pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ClassicTensor {
    /// Unique identifier for the tensor
    tensor_id: field_types::TensorId,

    /// Example: `[h, w, 3]` for an RGB image, stored in row-major-order.
    /// The order matches that of numpy etc, and is ordered so that
    /// the "tighest wound" dimension is last.
    ///
    /// An empty shape means this tensor is a scale, i.e. of length 1.
    /// An empty vector has shape `[0]`, an empty matrix shape `[0, 0]`, etc.
    ///
    /// Conceptually `[h,w]` == `[h,w,1]` == `[h,w,1,1,1]` etc in most circumstances.
    shape: Vec<field_types::TensorDimension>,

    /// The per-element data format.
    /// numpy calls this `dtype`.
    pub dtype: TensorDataType,

    /// The per-element data meaning
    /// Used to indicated if the data should be interpreted as color, class_id, etc.
    pub meaning: field_types::TensorDataMeaning,

    /// The actual contents of the tensor.
    pub data: TensorDataStore,
}

impl field_types::TensorTrait for ClassicTensor {
    fn id(&self) -> field_types::TensorId {
        self.tensor_id
    }

    fn shape(&self) -> &[field_types::TensorDimension] {
        self.shape.as_slice()
    }

    fn num_dim(&self) -> usize {
        self.num_dim()
    }

    fn is_shaped_like_an_image(&self) -> bool {
        self.is_shaped_like_an_image()
    }

    fn is_vector(&self) -> bool {
        self.is_vector()
    }

    fn meaning(&self) -> field_types::TensorDataMeaning {
        self.meaning
    }

    fn get(&self, index: &[u64]) -> Option<TensorElement> {
        self.get(index)
    }
}

impl ClassicTensor {
    pub fn new(
        tensor_id: field_types::TensorId,
        shape: Vec<field_types::TensorDimension>,
        dtype: TensorDataType,
        meaning: field_types::TensorDataMeaning,
        data: TensorDataStore,
    ) -> Self {
        Self {
            tensor_id,
            shape,
            dtype,
            meaning,
            data,
        }
    }

    #[inline]
    pub fn id(&self) -> field_types::TensorId {
        self.tensor_id
    }

    #[inline]
    pub fn shape(&self) -> &[field_types::TensorDimension] {
        self.shape.as_slice()
    }

    #[inline]
    pub fn dtype(&self) -> TensorDataType {
        self.dtype
    }

    #[inline]
    pub fn meaning(&self) -> field_types::TensorDataMeaning {
        self.meaning
    }

    #[inline]
    pub fn data<A: bytemuck::Pod + TensorDataTypeTrait>(&self) -> Option<&[A]> {
        self.data.as_slice()
    }

    /// True if the shape has a zero in it anywhere.
    ///
    /// Note that `shape=[]` means this tensor is a scalar, and thus NOT empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.shape.iter().any(|d| d.size == 0)
    }

    /// Number of elements (the product of [`Self::shape`]).
    ///
    /// NOTE: Returns `1` for scalars (shape=[]).
    pub fn len(&self) -> u64 {
        let mut len = 1;
        for dim in &self.shape {
            len = dim.size.saturating_mul(len);
        }
        len
    }

    /// Number of dimensions. Same as length of [`Self::shape`].
    #[inline]
    pub fn num_dim(&self) -> usize {
        self.shape.len()
    }

    /// Shape is one of `[N]`, `[1, N]` or `[N, 1]`
    pub fn is_vector(&self) -> bool {
        let shape = &self.shape;
        shape.len() == 1 || { shape.len() == 2 && (shape[0].size == 1 || shape[1].size == 1) }
    }

    pub fn is_shaped_like_an_image(&self) -> bool {
        self.num_dim() == 2
            || self.num_dim() == 3 && {
                matches!(
                    self.shape.last().unwrap().size,
                    // gray, rgb, rgba
                    1 | 3 | 4
                )
            }
    }

    /// The index must be the same length as the dimension.
    ///
    /// `None` if out of bounds, or if [`Self::data`] is not [`TensorDataStore::Dense`].
    ///
    /// Example: `tensor.get(&[y, x])` to sample a depth image.
    /// NOTE: we use numpy ordering of the arguments! Most significant first!
    pub fn get(&self, index: &[u64]) -> Option<TensorElement> {
        if index.len() != self.shape.len() {
            return None;
        }

        match &self.data {
            TensorDataStore::Dense(bytes) => {
                let mut stride = self.dtype.size();
                let mut offset = 0;
                for (field_types::TensorDimension { size, name: _ }, index) in
                    self.shape.iter().zip(index).rev()
                {
                    if size <= index {
                        return None;
                    }
                    offset += index * stride;
                    stride *= size;
                }
                if stride != bytes.len() as u64 {
                    return None; // Bad tensor
                }

                let begin = offset as usize;
                let end = (offset + self.dtype.size()) as usize;
                let data = &bytes[begin..end];

                Some(match self.dtype {
                    TensorDataType::U8 => TensorElement::U8(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::U16 => TensorElement::U16(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::U32 => TensorElement::U32(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::U64 => TensorElement::U64(bytemuck::pod_read_unaligned(data)),

                    TensorDataType::I8 => TensorElement::I8(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::I16 => TensorElement::I16(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::I32 => TensorElement::I32(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::I64 => TensorElement::I64(bytemuck::pod_read_unaligned(data)),

                    TensorDataType::F16 => TensorElement::F16(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::F32 => TensorElement::F32(bytemuck::pod_read_unaligned(data)),
                    TensorDataType::F64 => TensorElement::F64(bytemuck::pod_read_unaligned(data)),
                })
            }
            TensorDataStore::Jpeg(_) => None, // Too expensive to unpack here.
        }
    }
}
