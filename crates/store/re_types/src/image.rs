//! Image-related utilities.

use re_types_core::ArrowBuffer;
use smallvec::{smallvec, SmallVec};

use crate::{
    datatypes::ChannelDatatype,
    datatypes::{Blob, TensorBuffer, TensorData},
};

// ----------------------------------------------------------------------------

/// The kind of image data, either color, segmentation, or depth image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageKind {
    /// A normal grayscale or color image ([`crate::archetypes::Image`]).
    Color,

    /// A depth map ([`crate::archetypes::DepthImage`]).
    Depth,

    /// A segmentation image ([`crate::archetypes::SegmentationImage`]).
    ///
    /// The data is a [`crate::components::ClassId`] which should be
    /// looked up using the appropriate [`crate::components::AnnotationContext`]
    Segmentation,
}

// ----------------------------------------------------------------------------

/// Errors when converting images from the [`image`] crate to an [`crate::archetypes::Image`].
#[cfg(feature = "image")]
#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageConversionError {
    /// Unknown color type from the image crate.
    ///
    /// This should only happen if you are using a newer `image` crate than the one Rerun was built for,
    /// because `image` can add new color types without it being a breaking change,
    /// so we cannot exhaustively match on all color types.
    #[error("Unsupported color type: {0:?}. We support 8-bit, 16-bit, and f32 images, and RGB, RGBA, Luminance, and Luminance-Alpha.")]
    UnsupportedImageColorType(image::ColorType),
}

/// Errors when loading image files.
#[cfg(feature = "image")]
#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageLoadError {
    /// e.g. failed to decode a JPEG file.
    #[error(transparent)]
    Image(std::sync::Arc<image::ImageError>),

    /// e.g. failed to find a file on disk.
    #[error("Failed to load file: {0}")]
    ReadError(std::sync::Arc<std::io::Error>),

    /// Failure to convert the loaded image to a [`crate::archetypes::Image`].
    #[error(transparent)]
    ImageConversionError(#[from] ImageConversionError),

    /// The encountered MIME type is not supported for decoding images.
    #[error("MIME type '{0}' is not supported for images")]
    UnsupportedMimeType(String),

    /// Failed to read the MIME type from inspecting the image data blob.
    #[error("Could not detect MIME type from the image contents")]
    UnrecognizedMimeType,
}

#[cfg(feature = "image")]
impl From<image::ImageError> for ImageLoadError {
    #[inline]
    fn from(err: image::ImageError) -> Self {
        Self::Image(std::sync::Arc::new(err))
    }
}

#[cfg(feature = "image")]
impl From<std::io::Error> for ImageLoadError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        Self::ReadError(std::sync::Arc::new(err))
    }
}

// ----------------------------------------------------------------------------

/// Error returned when trying to interpret a tensor as an image.
#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageConstructionError<T: TryInto<TensorData>>
where
    T::Error: std::error::Error,
{
    /// Could not convert source to [`TensorData`].
    #[error("Could not convert source to TensorData: {0}")]
    TensorDataConversion(T::Error),

    /// The tensor did not have the right shape for an image (e.g. had too many dimensions).
    #[error("Could not create Image from TensorData with shape {0:?}")]
    BadImageShape(ArrowBuffer<u64>),

    /// Happens if you try to cast `NV12` or `YUY2` to a depth image or segmentation image.
    #[error("Chroma downsampling is not supported for this image type (e.g. DepthImage or SegmentationImage)")]
    ChromaDownsamplingNotSupported,
}

/// Converts it to what is useful for the image API.
pub fn blob_and_datatype_from_tensor(tensor_buffer: TensorBuffer) -> (Blob, ChannelDatatype) {
    match tensor_buffer {
        TensorBuffer::U8(buffer) => (Blob(buffer), ChannelDatatype::U8),
        TensorBuffer::U16(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::U16),
        TensorBuffer::U32(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::U32),
        TensorBuffer::U64(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::U64),
        TensorBuffer::I8(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::I8),
        TensorBuffer::I16(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::I16),
        TensorBuffer::I32(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::I32),
        TensorBuffer::I64(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::I64),
        TensorBuffer::F16(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::F16),
        TensorBuffer::F32(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::F32),
        TensorBuffer::F64(buffer) => (Blob(buffer.cast_to_u8()), ChannelDatatype::F64),
    }
}

// ----------------------------------------------------------------------------

/// Types that implement this can be used as image channel types.
///
/// Implemented for `u8, u16, u32, u64, i8, i16, i32, i64, f16, f32, f64`.
pub trait ImageChannelType: bytemuck::Pod {
    /// The [`ChannelDatatype`] for this type.
    const CHANNEL_TYPE: ChannelDatatype;
}

impl ImageChannelType for u8 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::U8;
}

impl ImageChannelType for u16 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::U16;
}

impl ImageChannelType for u32 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::U32;
}

impl ImageChannelType for u64 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::U64;
}

impl ImageChannelType for i8 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::I8;
}

impl ImageChannelType for i16 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::I16;
}

impl ImageChannelType for i32 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::I32;
}

impl ImageChannelType for i64 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::I64;
}

impl ImageChannelType for half::f16 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::F16;
}

impl ImageChannelType for f32 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::F32;
}

impl ImageChannelType for f64 {
    const CHANNEL_TYPE: ChannelDatatype = ChannelDatatype::F64;
}

// ----------------------------------------------------------------------------

/// Returns the indices of an appropriate set of dimensions.
///
/// Ignores leading and trailing 1-sized dimensions.
///
/// For instance: `[1, 480, 640, 3, 1]` would return `[1, 2, 3]`,
/// the indices of the `[480, 640, 3]` dimensions.
pub fn find_non_empty_dim_indices(shape: &[u64]) -> SmallVec<[usize; 4]> {
    match shape.len() {
        0 => return smallvec![],
        1 => return smallvec![0],
        2 => return smallvec![0, 1],
        _ => {}
    }

    // Find a range of non-unit dimensions.
    // [1, 1, 1, 480, 640, 3, 1, 1, 1]
    //           ^---------^   goal range

    let mut non_unit_indices =
        shape
            .iter()
            .enumerate()
            .filter_map(|(ind, &dim)| if dim != 1 { Some(ind) } else { None });

    // 0 is always a valid index.
    let mut min = non_unit_indices.next().unwrap_or(0);
    let mut max = non_unit_indices.last().unwrap_or(min);

    // Note, these are inclusive ranges.

    // First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    // Grow to a min-size of 2.
    // (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while max == min && max + 1 < shape.len() {
        max += 1;
    }

    // Next, consider empty outer dimensions if we still need them.
    // Grow up to 3 if the inner dimension is already 3 or 4 (Color Images)
    // Otherwise, only grow up to 2.
    // (1x1x3) -> 1x1x3 rgb rather than 1x3 mono
    let target_len = match shape[max] {
        3 | 4 => 3,
        _ => 2,
    };

    while max - min + 1 < target_len && 0 < min {
        min -= 1;
    }

    (min..=max).collect()
}

#[test]
fn test_find_non_empty_dim_indices() {
    fn expect(shape: &[u64], expected: &[usize]) {
        let got = find_non_empty_dim_indices(shape);
        assert!(
            got.as_slice() == expected,
            "Input: {shape:?}, got {got:?}, expected {expected:?}"
        );
    }

    expect(&[], &[]);
    expect(&[0], &[0]);
    expect(&[1], &[0]);
    expect(&[100], &[0]);

    expect(&[480, 640], &[0, 1]);
    expect(&[480, 640, 1], &[0, 1]);
    expect(&[480, 640, 1, 1], &[0, 1]);
    expect(&[480, 640, 3], &[0, 1, 2]);
    expect(&[1, 480, 640], &[1, 2]);
    expect(&[1, 480, 640, 3, 1], &[1, 2, 3]);
    expect(&[1, 3, 480, 640, 1], &[1, 2, 3]);
    expect(&[1, 1, 480, 640], &[2, 3]);
    expect(&[1, 1, 480, 640, 1, 1], &[2, 3]);

    expect(&[1, 1, 3], &[0, 1, 2]);
    expect(&[1, 1, 3, 1], &[2, 3]);
}

// ----------------------------------------------------------------------------

// TODO(andreas): Expose this in the API?
/// Yuv matrix coefficients that determine how a YUV image is meant to be converted to RGB.
///
/// A rigorious definition of the yuv conversion matrix would still require to define
/// the transfer characteristics & color primaries of the resulting RGB space.
/// See [`re_video::decode`]'s documentation.
///
/// However, at this point we generally assume that no further processing is needed after the transform.
/// This is acceptable for most non-HDR content because of the following properties of `Bt709`/`Bt601`/ sRGB:
/// * Bt709 & sRGB primaries are practically identical
/// * Bt601 PAL & Bt709 color primaries are the same (with some slight differences for Bt709 NTSC)
/// * Bt709 & sRGB transfer function are almost identical (and the difference is widely ignored)
///
/// (sources: <https://en.wikipedia.org/wiki/Rec._709>, <https://en.wikipedia.org/wiki/Rec._601>)
/// …which means for the moment we pretty much only care about the (actually quite) different YUV conversion matrices!
#[derive(Clone, Copy, Debug)]
pub enum YuvMatrixCoefficients {
    /// BT.601 (aka. SDTV, aka. Rec.601)
    ///
    /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion/>
    Bt601,

    /// BT.709 (aka. HDTV, aka. Rec.709)
    ///
    /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion/>
    ///
    /// These are the same primaries we usually assume and use for all of Rerun's rendering
    /// since they are the same primaries used by sRGB.
    /// <https://en.wikipedia.org/wiki/Rec._709#Relationship_to_sRGB/>
    /// The OETF/EOTF function (<https://en.wikipedia.org/wiki/Transfer_functions_in_imaging>) is different,
    /// but for all other purposes they are the same.
    /// (The only reason for us to convert to optical units ("linear" instead of "gamma") is for
    /// lighting computation & tonemapping where we typically start out with sRGB anyways!)
    Bt709,
    //
    // Not yet supported. These vary a lot more from the other two!
    //
    // /// BT.2020 (aka. PQ, aka. Rec.2020)
    // ///
    // /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.2020_conversion/>
    // BT2020_ConstantLuminance,
    // BT2020_NonConstantLuminance,
}

/// Returns sRGB from YUV color.
///
/// This conversion mirrors the function of the same name in `yuv_converter.wgsl`
///
/// Specifying the color standard should be exposed in the future [#3541](https://github.com/rerun-io/rerun/pull/3541)
pub fn rgb_from_yuv(
    y: u8,
    u: u8,
    v: u8,
    limited_range: bool,
    coefficients: YuvMatrixCoefficients,
) -> [u8; 3] {
    let (mut y, mut u, mut v) = (y as f32, u as f32, v as f32);

    // rescale YUV values
    if limited_range {
        // Via https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion:
        // "The resultant signals range from 16 to 235 for Y′ (Cb and Cr range from 16 to 240);
        // the values from 0 to 15 are called footroom, while the values from 236 to 255 are called headroom."
        y = (y - 16.0) / 219.0;
        u = (u - 128.0) / 224.0;
        v = (v - 128.0) / 224.0;
    } else {
        y /= 255.0;
        u = (u - 128.0) / 255.0;
        v = (v - 128.0) / 255.0;
    }

    let r;
    let g;
    let b;

    match coefficients {
        YuvMatrixCoefficients::Bt601 => {
            // BT.601 (aka. SDTV, aka. Rec.601). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion
            r = y + 1.402 * v;
            g = y - 0.344 * u - 0.714 * v;
            b = y + 1.772 * u;
        }

        YuvMatrixCoefficients::Bt709 => {
            // BT.709 (aka. HDTV, aka. Rec.709). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion
            r = y + 1.575 * v;
            g = y - 0.187 * u - 0.468 * v;
            b = y + 1.856 * u;
        }
    }

    [(255.0 * r) as u8, (255.0 * g) as u8, (255.0 * b) as u8]
}
