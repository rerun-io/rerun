use crate::tensor_data::{TensorCastError, TensorDataType, TensorElement};

#[cfg(feature = "image")]
use crate::tensor_data::TensorImageLoadError;

#[allow(unused_imports)] // Used for docstring links
use crate::archetypes::EncodedImage;

use super::{TensorBuffer, TensorData, TensorDimension};

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
                let shape = value
                    .shape()
                    .iter()
                    .map(|dim| TensorDimension {
                        size: *dim as u64,
                        name: None,
                    })
                    .collect();

                let vec = if value.is_standard_layout() {
                    let (mut vec, offset) = value.into_raw_vec_and_offset();
                    // into_raw_vec_and_offset() guarantees that the logical element order (.iter()) matches the internal
                    // storage order in the returned vector if the array is in standard layout.
                    if let Some(offset) = offset {
                        vec.drain(..offset);
                        vec
                    } else {
                        debug_assert!(vec.is_empty());
                        vec
                    }
                } else {
                    value.into_iter().collect::<Vec<_>>()
                };

                Ok(Self {
                    shape,
                    buffer: TensorBuffer::$variant(vec.into()),
                })
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
            let (vec, offset) = value.into_raw_vec_and_offset();
            // into_raw_vec_and_offset() guarantees that the logical element order (.iter()) matches the internal
            // storage order in the returned vector if the array is in standard layout.
            let vec_slice = if let Some(offset) = offset {
                &vec[offset..]
            } else {
                debug_assert!(vec.is_empty());
                &vec
            };
            Ok(Self {
                shape,
                buffer: TensorBuffer::F16(Vec::from(bytemuck::cast_slice(vec_slice)).into()),
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
    /// [`EncodedImage::from_file`].
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
    /// [`EncodedImage::from_file_contents`].
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
