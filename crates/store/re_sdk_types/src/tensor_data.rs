//! Internal helpers; not part of the public API.
#![expect(missing_docs)]

use half::f16;

#[expect(unused_imports)] // Used for docstring links
use crate::datatypes::TensorData;

// ----------------------------------------------------------------------------

/// Errors when trying to cast [`TensorData`] to an `ndarray`
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

/// Errors when loading [`TensorData`] from the [`image`] crate.
#[cfg(feature = "image")]
#[derive(thiserror::Error, Clone, Debug)]
pub enum TensorImageLoadError {
    #[error(transparent)]
    Image(std::sync::Arc<image::ImageError>),

    #[error(
        "Unsupported color type: {0:?}. We support 8-bit, 16-bit, and f32 images, and RGB, RGBA, Luminance, and Luminance-Alpha."
    )]
    UnsupportedImageColorType(image::ColorType),

    #[error("Failed to load file: {0}")]
    ReadError(std::sync::Arc<std::io::Error>),

    #[error("The encoded tensor shape did not match its metadata {expected:?} != {found:?}")]
    InvalidMetaData { expected: Vec<u64>, found: Vec<u64> },
}

#[cfg(feature = "image")]
impl From<image::ImageError> for TensorImageLoadError {
    #[inline]
    fn from(err: image::ImageError) -> Self {
        Self::Image(std::sync::Arc::new(err))
    }
}

#[cfg(feature = "image")]
impl From<std::io::Error> for TensorImageLoadError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        Self::ReadError(std::sync::Arc::new(err))
    }
}

// ----------------------------------------------------------------------------

/// The data types supported by a [`crate::datatypes::TensorData`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// Is this datatype an integer?
    #[inline]
    pub fn is_integer(&self) -> bool {
        !self.is_float()
    }

    /// Is this datatype a floating point number?
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

    /// What is the minimum finite value representable by this datatype?
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

    /// What is the maximum finite value representable by this datatype?
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

/// The data that can be stored in a [`crate::datatypes::TensorData`].
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
    F16(half::f16),

    /// 32-bit floating point number.
    F32(f32),

    /// 64-bit floating point number.
    F64(f64),
}

impl TensorElement {
    /// Get the value as a 64-bit floating point number.
    ///
    /// Note that this may cause rounding for large 64-bit integers,
    /// as `f64` can only represent integers up to 2^53 exactly.
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

    /// Convert the value to a `u16`, but only if it can be represented
    /// exactly as a `u16`, without any rounding or clamping.
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

    /// Format the value with `re_format`
    pub fn format(&self) -> String {
        match self {
            Self::U8(val) => re_format::format_uint(*val),
            Self::U16(val) => re_format::format_uint(*val),
            Self::U32(val) => re_format::format_uint(*val),
            Self::U64(val) => re_format::format_uint(*val),
            Self::I8(val) => re_format::format_int(*val),
            Self::I16(val) => re_format::format_int(*val),
            Self::I32(val) => re_format::format_int(*val),
            Self::I64(val) => re_format::format_int(*val),
            Self::F16(val) => re_format::format_f16(*val),
            Self::F32(val) => re_format::format_f32(*val),
            Self::F64(val) => re_format::format_f64(*val),
        }
    }

    /// Get the minimum value representable by this element's type.
    fn min_value(&self) -> Self {
        match self {
            Self::U8(_) => Self::U8(u8::MIN),
            Self::U16(_) => Self::U16(u16::MIN),
            Self::U32(_) => Self::U32(u32::MIN),
            Self::U64(_) => Self::U64(u64::MIN),

            Self::I8(_) => Self::I8(i8::MIN),
            Self::I16(_) => Self::I16(i16::MIN),
            Self::I32(_) => Self::I32(i32::MIN),
            Self::I64(_) => Self::I64(i64::MIN),

            Self::F16(_) => Self::F16(f16::MIN),
            Self::F32(_) => Self::F32(f32::MIN),
            Self::F64(_) => Self::F64(f64::MIN),
        }
    }

    /// Get the maximum value representable by this element's type.
    fn max_value(&self) -> Self {
        match self {
            Self::U8(_) => Self::U8(u8::MAX),
            Self::U16(_) => Self::U16(u16::MAX),
            Self::U32(_) => Self::U32(u32::MAX),
            Self::U64(_) => Self::U64(u64::MAX),

            Self::I8(_) => Self::I8(i8::MAX),
            Self::I16(_) => Self::I16(i16::MAX),
            Self::I32(_) => Self::I32(i32::MAX),
            Self::I64(_) => Self::I64(i64::MAX),

            Self::F16(_) => Self::F16(f16::MAX),
            Self::F32(_) => Self::F32(f32::MAX),
            Self::F64(_) => Self::F64(f64::MAX),
        }
    }

    /// Formats the element as a string, padded to the width of the largest possible value.
    pub fn format_padded(&self) -> String {
        let max_len = match self {
            Self::U8(_) | Self::U16(_) | Self::U32(_) | Self::U64(_) => {
                self.max_value().format().chars().count()
            }
            Self::I8(_) | Self::I16(_) | Self::I32(_) | Self::I64(_) => {
                self.min_value().format().chars().count()
            }
            // These were determined by checking the length of random formatted values
            Self::F16(_) | Self::F32(_) => 12,
            Self::F64(_) => 22,
        };
        let value_str = self.format();
        format!("{value_str:>max_len$}")
    }
}

impl std::fmt::Display for TensorElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::U8(elem) => std::fmt::Display::fmt(elem, f),
            Self::U16(elem) => std::fmt::Display::fmt(elem, f),
            Self::U32(elem) => std::fmt::Display::fmt(elem, f),
            Self::U64(elem) => std::fmt::Display::fmt(elem, f),
            Self::I8(elem) => std::fmt::Display::fmt(elem, f),
            Self::I16(elem) => std::fmt::Display::fmt(elem, f),
            Self::I32(elem) => std::fmt::Display::fmt(elem, f),
            Self::I64(elem) => std::fmt::Display::fmt(elem, f),
            Self::F16(elem) => std::fmt::Display::fmt(elem, f),
            Self::F32(elem) => std::fmt::Display::fmt(elem, f),
            Self::F64(elem) => std::fmt::Display::fmt(elem, f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_element_format() {
        let elem = TensorElement::U8(42);
        assert_eq!(elem.format(), "42");

        let elem = TensorElement::F32(3.17);
        assert_eq!(elem.format(), "3.17");

        let elem = TensorElement::I64(-123456789);
        assert_eq!(elem.format(), "−123\u{2009}456\u{2009}789");
    }

    #[test]
    fn test_tensor_element_format_padded() {
        macro_rules! test_padded_format {
            ($type:ident, $random:expr) => {
                let type_name = stringify!($type);
                let left_padded = TensorElement::$type($random).format_padded();
                for _ in 0..100 {
                    let elem = TensorElement::$type($random);
                    let right_padded = elem.format_padded();
                    assert_eq!(
                        left_padded.chars().count(),
                        right_padded.chars().count(),
                        "Padded format length mismatch for type {type_name} with value '{left_padded}' and value '{right_padded}'",
                    );
                }
            };
        }
        test_padded_format!(U8, rand::random());
        test_padded_format!(U16, rand::random());
        test_padded_format!(U32, rand::random());
        test_padded_format!(U64, rand::random());
        test_padded_format!(I8, rand::random());
        test_padded_format!(I16, rand::random());
        test_padded_format!(I32, rand::random());
        test_padded_format!(I64, rand::random());

        test_padded_format!(F16, f16::from_bits(rand::random()));
        test_padded_format!(F32, f32::from_bits(rand::random()));
        test_padded_format!(F64, f64::from_bits(rand::random()));
    }
}
