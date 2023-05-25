use half::f16;

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

/// The data types supported by a [`crate::component_types::Tensor`].
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

    #[inline]
    pub fn is_integer(&self) -> bool {
        !self.is_float()
    }

    #[inline]
    pub fn is_float(&self) -> bool {
        match self {
            Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64 => false,
            Self::F16 | Self::F32 | Self::F64 => true,
        }
    }

    #[inline]
    pub fn min_value(&self) -> f64 {
        match self {
            Self::U8 => u8::MIN as _,
            Self::U16 => u16::MIN as _,
            Self::U32 => u32::MIN as _,
            Self::U64 => u64::MIN as _,

            Self::I8 => i8::MIN as _,
            Self::I16 => i16::MIN as _,
            Self::I32 => i32::MIN as _,
            Self::I64 => i64::MIN as _,

            Self::F16 => f16::MIN.into(),
            Self::F32 => f32::MIN as _,
            Self::F64 => f64::MIN,
        }
    }

    #[inline]
    pub fn max_value(&self) -> f64 {
        match self {
            Self::U8 => u8::MAX as _,
            Self::U16 => u16::MAX as _,
            Self::U32 => u32::MAX as _,
            Self::U64 => u64::MAX as _,

            Self::I8 => i8::MAX as _,
            Self::I16 => i16::MAX as _,
            Self::I32 => i32::MAX as _,
            Self::I64 => i64::MAX as _,

            Self::F16 => f16::MAX.into(),
            Self::F32 => f32::MAX as _,
            Self::F64 => f64::MAX,
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

/// The data that can be stored in a [`crate::component_types::Tensor`].
#[derive(Clone, Copy, Debug, PartialEq)]
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
    F16(arrow2::types::f16),

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

            Self::F16(value) => value.to_f32() as _,
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

            Self::F16(value) => u16_from_f64(value.to_f32() as f64),
            Self::F32(value) => u16_from_f64(*value as f64),
            Self::F64(value) => u16_from_f64(*value),
        }
    }
}

impl std::fmt::Display for TensorElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TensorElement::U8(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::U16(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::U32(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::U64(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::I8(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::I16(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::I32(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::I64(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::F16(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::F32(elem) => std::fmt::Display::fmt(elem, f),
            TensorElement::F64(elem) => std::fmt::Display::fmt(elem, f),
        }
    }
}
