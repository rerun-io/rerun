use half::f16;

use crate::datatypes::{TensorBuffer, TensorData};

#[cfg(feature = "image")]
use crate::datatypes::TensorDimension;

// Much of the following duplicates code from: `crates/re_components/src/tensor.rs`, which
// will eventually go away as the Tensor migration is completed.

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

    #[error("Expected a HxW, HxWx1 or HxWx3 tensor, but got {0:?}")]
    UnexpectedJpegShape(Vec<TensorDimension>),

    #[error("Unsupported color type: {0:?}. We support 8-bit, 16-bit, and f32 images, and RGB, RGBA, Luminance, and Luminance-Alpha.")]
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

/// Errors when converting [`TensorData`] to [`image`] images.
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

// ----------------------------------------------------------------------------

/// A thin wrapper around a [`TensorData`] that is guaranteed to not be compressed (never a jpeg).
///
/// All clones are shallow, like for [`TensorData`].
#[derive(Clone)]
pub struct DecodedTensor(TensorData);

impl DecodedTensor {
    #[inline(always)]
    pub fn inner(&self) -> &TensorData {
        &self.0
    }

    #[inline(always)]
    pub fn into_inner(self) -> TensorData {
        self.0
    }
}

// Backwards comparabillity shim
// TODO(jleibs): fully express this in terms of indicator components
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorDataMeaning {
    /// Default behavior: guess based on shape
    Unknown,

    /// The data is an annotated [`crate::components::ClassId`] which should be
    /// looked up using the appropriate [`crate::components::AnnotationContext`]
    ClassId,

    /// Image data interpreted as depth map.
    Depth,
}

impl TryFrom<TensorData> for DecodedTensor {
    type Error = TensorData;

    fn try_from(tensor: TensorData) -> Result<Self, TensorData> {
        match &tensor.buffer {
            TensorBuffer::U8(_)
            | TensorBuffer::U16(_)
            | TensorBuffer::U32(_)
            | TensorBuffer::U64(_)
            | TensorBuffer::I8(_)
            | TensorBuffer::I16(_)
            | TensorBuffer::I32(_)
            | TensorBuffer::I64(_)
            | TensorBuffer::F16(_)
            | TensorBuffer::F32(_)
            | TensorBuffer::F64(_) => Ok(Self(tensor)),
            TensorBuffer::Jpeg(_) | TensorBuffer::Nv12(_) | TensorBuffer::Yuy2(_) => Err(tensor),
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

        let (depth, buffer) = match image {
            image::DynamicImage::ImageLuma8(image) => {
                (1, TensorBuffer::U8(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgb8(image) => (3, TensorBuffer::U8(image.into_raw().into())),
            image::DynamicImage::ImageRgba8(image) => {
                (4, TensorBuffer::U8(image.into_raw().into()))
            }
            image::DynamicImage::ImageLuma16(image) => {
                (1, TensorBuffer::U16(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgb16(image) => {
                (3, TensorBuffer::U16(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgba16(image) => {
                (4, TensorBuffer::U16(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgb32F(image) => {
                (3, TensorBuffer::F32(image.into_raw().into()))
            }
            image::DynamicImage::ImageRgba32F(image) => {
                (4, TensorBuffer::F32(image.into_raw().into()))
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
        let shape = if depth == 1 {
            vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
            ]
        } else {
            vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
                TensorDimension::depth(depth),
            ]
        };
        let tensor = TensorData { shape, buffer };
        Ok(DecodedTensor(tensor))
    }

    pub fn try_decode(maybe_encoded_tensor: TensorData) -> Result<Self, TensorImageLoadError> {
        match &maybe_encoded_tensor.buffer {
            TensorBuffer::U8(_)
            | TensorBuffer::U16(_)
            | TensorBuffer::U32(_)
            | TensorBuffer::U64(_)
            | TensorBuffer::I8(_)
            | TensorBuffer::I16(_)
            | TensorBuffer::I32(_)
            | TensorBuffer::I64(_)
            | TensorBuffer::F16(_)
            | TensorBuffer::F32(_)
            | TensorBuffer::F64(_)
            | TensorBuffer::Nv12(_)
            | TensorBuffer::Yuy2(_) => Ok(Self(maybe_encoded_tensor)), // Decoding happens on the GPU

            TensorBuffer::Jpeg(jpeg_bytes) => {
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
        jpeg_bytes: &[u8],
        [expected_height, expected_width, expected_channels]: [u64; 3],
    ) -> Result<DecodedTensor, TensorImageLoadError> {
        re_tracing::profile_function!(format!("{expected_width}x{expected_height}"));

        use image::io::Reader as ImageReader;
        let mut reader = ImageReader::new(std::io::Cursor::new(jpeg_bytes));
        reader.set_format(image::ImageFormat::Jpeg);
        let img = {
            re_tracing::profile_scope!("decode_jpeg");
            reader.decode()?
        };

        let (w, h) = (img.width() as u64, img.height() as u64);
        let channels = img.color().channel_count() as u64;

        if (w, h, channels) != (expected_width, expected_height, expected_channels) {
            return Err(TensorImageLoadError::InvalidMetaData {
                expected: [expected_height, expected_width, expected_channels].into(),
                found: [h, w, channels].into(),
            });
        }

        Self::from_image(img)
    }
}

impl AsRef<TensorData> for DecodedTensor {
    #[inline(always)]
    fn as_ref(&self) -> &TensorData {
        &self.0
    }
}

impl std::ops::Deref for DecodedTensor {
    type Target = TensorData;

    #[inline(always)]
    fn deref(&self) -> &TensorData {
        &self.0
    }
}

impl std::borrow::Borrow<TensorData> for DecodedTensor {
    #[inline(always)]
    fn borrow(&self) -> &TensorData {
        &self.0
    }
}
