use crate::tensor_data::{TensorCastError, TensorDataType, TensorElement};

#[cfg(feature = "image")]
use crate::tensor_data::{TensorImageLoadError, TensorImageSaveError};

#[allow(unused_imports)] // Used for docstring links
use crate::archetypes::ImageEncoded;

use super::{TensorBuffer, TensorData, TensorDimension};

// Much of the following duplicates code from: `crates/re_components/src/tensor.rs`, which
// will eventually go away as the Tensor migration is completed.

// ----------------------------------------------------------------------------

impl TensorData {
    /// Create a new tensor.
    #[inline]
    pub fn new(shape: Vec<TensorDimension>, buffer: TensorBuffer) -> Self {
        Self { shape, buffer }
    }

    /// The shape of the tensor, including optional dimension names.
    #[inline]
    pub fn shape(&self) -> &[TensorDimension] {
        self.shape.as_slice()
    }

    /// Returns the shape of the tensor with all leading & trailing dimensions of size 1 ignored.
    ///
    /// If all dimension sizes are one, this returns only the first dimension.
    #[inline]
    pub fn shape_short(&self) -> &[TensorDimension] {
        if self.shape.is_empty() {
            &self.shape
        } else {
            let first_not_one = self.shape.iter().position(|dim| dim.size != 1);
            let last_not_one = self.shape.iter().rev().position(|dim| dim.size != 1);
            &self.shape[first_not_one.unwrap_or(0)..self.shape.len() - last_not_one.unwrap_or(0)]
        }
    }

    /// The number of dimensions of the tensor.
    ///
    /// An image tensor will usually have two (height, width) or three (height, width, channels) dimensions.
    #[inline]
    pub fn num_dim(&self) -> usize {
        self.shape.len()
    }

    /// If the tensor can be interpreted as an image, return the height, width, and channels/depth of it.
    pub fn image_height_width_channels(&self) -> Option<[u64; 3]> {
        let mut shape_short = self.shape.as_slice();

        // Ignore trailing dimensions of size 1:
        while 2 < shape_short.len() && shape_short.last().map_or(false, |d| d.size == 1) {
            shape_short = &shape_short[..shape_short.len() - 1];
        }

        // If the trailing dimension looks like a channel we ignore leading dimensions of size 1 down to
        // a minimum of 3 dimensions. Otherwise we ignore leading dimensions of size 1 down to 2 dimensions.
        let shrink_to = if shape_short
            .last()
            .map_or(false, |d| matches!(d.size, 1 | 3 | 4))
        {
            3
        } else {
            2
        };

        while shrink_to < shape_short.len() && shape_short.first().map_or(false, |d| d.size == 1) {
            shape_short = &shape_short[1..];
        }

        // TODO(emilk): check dimension names against our standard dimension names ("height", "width", "depth")

        match &self.buffer {
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
            | TensorBuffer::F64(_) => {
                match shape_short.len() {
                    1 => {
                        // Special case: Nx1(x1x1x …) tensors are treated as Nx1 gray images.
                        // Special case: Nx1(x1x1x …) tensors are treated as Nx1 gray images.
                        if self.shape.len() >= 2 {
                            Some([shape_short[0].size, 1, 1])
                        } else {
                            None
                        }
                    }
                    2 => Some([shape_short[0].size, shape_short[1].size, 1]),
                    3 => {
                        let channels = shape_short[2].size;
                        if matches!(channels, 1 | 3 | 4) {
                            // mono, rgb, rgba
                            Some([shape_short[0].size, shape_short[1].size, channels])
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
        }
    }

    /// Returns true if the tensor can be interpreted as an image.
    #[inline]
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

    /// Query with x, y, channel indices.
    ///
    /// Allows to query values for any image-like tensor even if it has more or less dimensions than 3.
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

    /// Get the value of the element at the given index.
    ///
    /// Return `None` if out-of-bounds.
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

        match &self.buffer {
            TensorBuffer::U8(buf) => Some(TensorElement::U8(buf[offset])),
            TensorBuffer::U16(buf) => Some(TensorElement::U16(buf[offset])),
            TensorBuffer::U32(buf) => Some(TensorElement::U32(buf[offset])),
            TensorBuffer::U64(buf) => Some(TensorElement::U64(buf[offset])),
            TensorBuffer::I8(buf) => Some(TensorElement::I8(buf[offset])),
            TensorBuffer::I16(buf) => Some(TensorElement::I16(buf[offset])),
            TensorBuffer::I32(buf) => Some(TensorElement::I32(buf[offset])),
            TensorBuffer::I64(buf) => Some(TensorElement::I64(buf[offset])),
            TensorBuffer::F16(buf) => Some(TensorElement::F16(buf[offset])),
            TensorBuffer::F32(buf) => Some(TensorElement::F32(buf[offset])),
            TensorBuffer::F64(buf) => Some(TensorElement::F64(buf[offset])),
        }
    }

    /// The datatype of the tensor.
    #[inline]
    pub fn dtype(&self) -> TensorDataType {
        self.buffer.dtype()
    }

    /// The size of the tensor data, in bytes.
    #[inline]
    pub fn size_in_bytes(&self) -> usize {
        self.buffer.size_in_bytes()
    }
}

impl Default for TensorData {
    #[inline]
    fn default() -> Self {
        Self {
            shape: Vec::new(),
            buffer: TensorBuffer::U8(Vec::new().into()),
        }
    }
}

// ----------------------------------------------------------------------------

macro_rules! ndarray_from_tensor {
    ($type:ty, $variant:ident) => {
        impl<'a> TryFrom<&'a TensorData> for ::ndarray::ArrayViewD<'a, $type> {
            type Error = TensorCastError;

            fn try_from(value: &'a TensorData) -> Result<Self, Self::Error> {
                let shape: Vec<_> = value.shape.iter().map(|d| d.size as usize).collect();

                if let TensorBuffer::$variant(data) = &value.buffer {
                    ndarray::ArrayViewD::from_shape(shape, data.as_slice())
                        .map_err(|err| TensorCastError::BadTensorShape { source: err })
                } else {
                    Err(TensorCastError::TypeMismatch)
                }
            }
        }
    };
}

macro_rules! tensor_from_ndarray {
    ($type:ty, $variant:ident) => {
        impl<'a, D: ::ndarray::Dimension> TryFrom<::ndarray::ArrayView<'a, $type, D>>
            for TensorData
        {
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
                    Some(slice) => Ok(TensorData {
                        shape,
                        buffer: TensorBuffer::$variant(Vec::from(slice).into()),
                    }),
                    None => Ok(TensorData {
                        shape,
                        buffer: TensorBuffer::$variant(
                            view.iter().cloned().collect::<Vec<_>>().into(),
                        ),
                    }),
                }
            }
        }

        impl<D: ::ndarray::Dimension> TryFrom<::ndarray::Array<$type, D>> for TensorData {
            type Error = TensorCastError;

            fn try_from(value: ndarray::Array<$type, D>) -> Result<Self, Self::Error> {
                let value = value.as_standard_layout();
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
                    .then(|| TensorData {
                        shape,
                        buffer: TensorBuffer::$variant(value.to_owned().into_raw_vec().into()),
                    })
                    .ok_or(TensorCastError::NotContiguousStdOrder)
            }
        }

        impl From<Vec<$type>> for TensorData {
            fn from(vec: Vec<$type>) -> Self {
                TensorData {
                    shape: vec![TensorDimension::unnamed(vec.len() as u64)],
                    buffer: TensorBuffer::$variant(vec.into()),
                }
            }
        }

        impl From<&[$type]> for TensorData {
            fn from(slice: &[$type]) -> Self {
                TensorData {
                    shape: vec![TensorDimension::unnamed(slice.len() as u64)],
                    buffer: TensorBuffer::$variant(slice.into()),
                }
            }
        }
    };
}

macro_rules! tensor_type {
    ($type:ty, $variant:ident) => {
        ndarray_from_tensor!($type, $variant);
        tensor_from_ndarray!($type, $variant);
    };
}

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

tensor_from_ndarray!(u8, U8);

// Manual expansion of ndarray_from_tensor! macro for `u8` types. We need to do this, because u8 can store encoded data
impl<'a> TryFrom<&'a TensorData> for ::ndarray::ArrayViewD<'a, u8> {
    type Error = TensorCastError;

    fn try_from(value: &'a TensorData) -> Result<Self, Self::Error> {
        match &value.buffer {
            TensorBuffer::U8(data) => {
                let shape: Vec<_> = value.shape.iter().map(|d| d.size as usize).collect();
                ndarray::ArrayViewD::from_shape(shape, bytemuck::cast_slice(data.as_slice()))
                    .map_err(|err| TensorCastError::BadTensorShape { source: err })
            }
            _ => Err(TensorCastError::TypeMismatch),
        }
    }
}

// Manual expansion of tensor_type! macro for `half::f16` types. We need to do this
// because arrow uses its own half type. The two use the same underlying representation
// but are still distinct types. `half::f16`, however, is more full-featured and
// generally a better choice to use when converting to ndarray.
// ==========================================
// TODO(jleibs): would be nice to support this with the macro definition as well
// but the bytemuck casts add a bit of complexity here.
impl<'a> TryFrom<&'a TensorData> for ::ndarray::ArrayViewD<'a, half::f16> {
    type Error = TensorCastError;

    fn try_from(value: &'a TensorData) -> Result<Self, Self::Error> {
        let shape: Vec<_> = value.shape.iter().map(|d| d.size as usize).collect();
        if let TensorBuffer::F16(data) = &value.buffer {
            ndarray::ArrayViewD::from_shape(shape, bytemuck::cast_slice(data.as_slice()))
                .map_err(|err| TensorCastError::BadTensorShape { source: err })
        } else {
            Err(TensorCastError::TypeMismatch)
        }
    }
}

impl<'a, D: ::ndarray::Dimension> TryFrom<::ndarray::ArrayView<'a, half::f16, D>> for TensorData {
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
            Some(slice) => Ok(Self {
                shape,
                buffer: TensorBuffer::F16(Vec::from(bytemuck::cast_slice(slice)).into()),
            }),
            None => Ok(Self {
                shape,
                buffer: TensorBuffer::F16(
                    view.iter()
                        .map(|f| arrow2::types::f16::from_bits(f.to_bits()))
                        .collect::<Vec<_>>()
                        .into(),
                ),
            }),
        }
    }
}

impl<D: ::ndarray::Dimension> TryFrom<::ndarray::Array<half::f16, D>> for TensorData {
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
        if value.is_standard_layout() {
            Ok(Self {
                shape,
                buffer: TensorBuffer::F16(
                    bytemuck::cast_slice(value.into_raw_vec().as_slice())
                        .to_vec()
                        .into(),
                ),
            })
        } else {
            Ok(Self {
                shape,
                buffer: TensorBuffer::F16(
                    value
                        .iter()
                        .map(|f| arrow2::types::f16::from_bits(f.to_bits()))
                        .collect::<Vec<_>>()
                        .into(),
                ),
            })
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "image")]
impl TensorData {
    /// Construct a tensor from the contents of an image file on disk.
    ///
    /// This will spend CPU cycles reading the file and decoding the image.
    /// To save CPU time and storage, we recommend you instead use
    /// [`ImageEncoded::from_file`].
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

        Self::from_image_bytes(&img_bytes, img_format)
    }

    /// Construct a tensor from the contents of an image file.
    ///
    /// This will spend CPU cycles decoding the image.
    /// To save CPU time and storage, we recommend you instead use
    /// [`ImageEncoded::from_file_contents`].
    ///
    /// Requires the `image` feature.
    #[inline]
    pub fn from_image_bytes(
        bytes: &[u8],
        format: image::ImageFormat,
    ) -> Result<Self, TensorImageLoadError> {
        re_tracing::profile_function!(format!("{format:?}"));
        let image = image::load_from_memory_with_format(bytes, format)?;
        Self::from_image(image)
    }

    /// Construct a tensor from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(image: impl Into<image::DynamicImage>) -> Result<Self, TensorImageLoadError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(image: image::DynamicImage) -> Result<Self, TensorImageLoadError> {
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
        Ok(Self { shape, buffer })
    }

    /// Predicts if [`Self::to_dynamic_image`] is likely to succeed, without doing anything expensive
    #[inline]
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

        let dyn_img_result = match (channels, &self.buffer) {
            (1, TensorBuffer::U8(buf)) => {
                GrayImage::from_raw(w, h, buf.to_vec()).map(DynamicImage::ImageLuma8)
            }
            (1, TensorBuffer::U16(buf)) => {
                Gray16Image::from_raw(w, h, buf.to_vec()).map(DynamicImage::ImageLuma16)
            }
            // TODO(emilk) f16
            (1, TensorBuffer::F32(buf)) => {
                let pixels = buf
                    .iter()
                    .map(|pixel| gamma_u8_from_linear_f32(*pixel))
                    .collect();
                GrayImage::from_raw(w, h, pixels).map(DynamicImage::ImageLuma8)
            }
            (1, TensorBuffer::F64(buf)) => {
                let pixels = buf
                    .iter()
                    .map(|&pixel| gamma_u8_from_linear_f32(pixel as f32))
                    .collect();
                GrayImage::from_raw(w, h, pixels).map(DynamicImage::ImageLuma8)
            }

            (3, TensorBuffer::U8(buf)) => {
                RgbImage::from_raw(w, h, buf.to_vec()).map(DynamicImage::ImageRgb8)
            }
            (3, TensorBuffer::U16(buf)) => {
                Rgb16Image::from_raw(w, h, buf.to_vec()).map(DynamicImage::ImageRgb16)
            }
            (3, TensorBuffer::F32(buf)) => {
                let pixels = buf.iter().copied().map(gamma_u8_from_linear_f32).collect();
                RgbImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgb8)
            }
            (3, TensorBuffer::F64(buf)) => {
                let pixels = buf
                    .iter()
                    .map(|&comp| gamma_u8_from_linear_f32(comp as f32))
                    .collect();
                RgbImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgb8)
            }

            (4, TensorBuffer::U8(buf)) => {
                RgbaImage::from_raw(w, h, buf.to_vec()).map(DynamicImage::ImageRgba8)
            }
            (4, TensorBuffer::U16(buf)) => {
                Rgba16Image::from_raw(w, h, buf.to_vec()).map(DynamicImage::ImageRgba16)
            }
            (4, TensorBuffer::F32(buf)) => {
                let rgba: &[[f32; 4]] = bytemuck::cast_slice(buf);
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
            (4, TensorBuffer::F64(buf)) => {
                let rgba: &[[f64; 4]] = bytemuck::cast_slice(buf);
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
                    self.buffer.dtype(),
                ))
            }
        };

        dyn_img_result.ok_or(TensorImageSaveError::BadData)
    }
}

#[cfg(feature = "image")]
impl TryFrom<image::DynamicImage> for TensorData {
    type Error = TensorImageLoadError;

    fn try_from(value: image::DynamicImage) -> Result<Self, Self::Error> {
        Self::from_image(value)
    }
}

#[cfg(feature = "image")]
impl<P: image::Pixel, S> TryFrom<image::ImageBuffer<P, S>> for TensorData
where
    image::DynamicImage: std::convert::From<image::ImageBuffer<P, S>>,
{
    type Error = TensorImageLoadError;

    fn try_from(value: image::ImageBuffer<P, S>) -> Result<Self, Self::Error> {
        Self::from_image(value)
    }
}

#[test]
fn test_image_height_width_channels() {
    let test_cases = [
        // Normal grayscale:
        (vec![1, 1, 480, 640, 1, 1], Some([480, 640, 1])),
        (vec![1, 1, 480, 640, 1], Some([480, 640, 1])),
        (vec![1, 1, 480, 640], Some([480, 640, 1])),
        (vec![1, 480, 640, 1, 1], Some([480, 640, 1])),
        (vec![1, 480, 640], Some([480, 640, 1])),
        (vec![480, 640, 1, 1], Some([480, 640, 1])),
        (vec![480, 640, 1], Some([480, 640, 1])),
        (vec![480, 640], Some([480, 640, 1])),
        //
        // Normal RGB:
        (vec![1, 1, 480, 640, 3, 1], Some([480, 640, 3])),
        (vec![1, 1, 480, 640, 3], Some([480, 640, 3])),
        (vec![1, 480, 640, 3, 1], Some([480, 640, 3])),
        (vec![480, 640, 3, 1], Some([480, 640, 3])),
        (vec![480, 640, 3], Some([480, 640, 3])),
        //
        // h=1, w=640, grayscale:
        (vec![1, 640], Some([1, 640, 1])),
        //
        // h=1, w=640, RGB:
        (vec![1, 640, 3], Some([1, 640, 3])),
        //
        // h=480, w=1, grayscale:
        (vec![480, 1], Some([480, 1, 1])),
        //
        // h=480, w=1, RGB:
        (vec![480, 1, 3], Some([480, 1, 3])),
        //
        // h=1, w=1, grayscale:
        (vec![1, 1], Some([1, 1, 1])),
        (vec![1, 1, 1], Some([1, 1, 1])),
        (vec![1, 1, 1, 1], Some([1, 1, 1])),
        //
        // h=1, w=1, RGB:
        (vec![1, 1, 3], Some([1, 1, 3])),
        (vec![1, 1, 1, 3], Some([1, 1, 3])),
        //
        // h=1, w=3, Mono:
        (vec![1, 3, 1], Some([1, 3, 1])),
        //
        // Ambiguous cases.
        //
        // These are here to show how the current implementation behaves, not to suggest that it is a
        // commitment to preserving this behavior going forward.
        // If you need to change this test, it's ok but we should still communicate the subtle change
        // in behavior.
        (vec![1, 1, 3, 1], Some([1, 1, 3])), // Could be [1, 3, 1]
        (vec![1, 3, 1, 1], Some([1, 3, 1])), // Could be [3, 1, 1]
    ];

    for (shape, expected_hwc) in test_cases {
        let tensor = TensorData::new(
            shape
                .iter()
                .map(|&size| TensorDimension::unnamed(size as u64))
                .collect(),
            TensorBuffer::U8(vec![0; shape.iter().product()].into()),
        );

        let hwc = tensor.image_height_width_channels();

        assert_eq!(
            hwc, expected_hwc,
            "Shape {shape:?} produced HWC {hwc:?}, but expected {expected_hwc:?}"
        );
    }
}
