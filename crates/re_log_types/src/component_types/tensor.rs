use arrow2::array::{FixedSizeBinaryArray, MutableFixedSizeBinaryArray};
use arrow2::buffer::Buffer;
use arrow2_convert::deserialize::ArrowDeserialize;
use arrow2_convert::field::ArrowField;
use arrow2_convert::{serialize::ArrowSerialize, ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;
use crate::{TensorDataType, TensorElement};

use super::arrow_convert_shims::BinaryBuffer;

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
    U8(BinaryBuffer),
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
    JPEG(BinaryBuffer),
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
            Self::F32(_) => TensorDataType::F32,
            Self::F64(_) => TensorDataType::F64,
        }
    }

    pub fn size_in_bytes(&self) -> usize {
        match self {
            Self::U8(buf) | Self::JPEG(buf) => buf.0.len(),
            Self::U16(buf) => buf.len(),
            Self::U32(buf) => buf.len(),
            Self::U64(buf) => buf.len(),
            Self::I8(buf) => buf.len(),
            Self::I16(buf) => buf.len(),
            Self::I32(buf) => buf.len(),
            Self::I64(buf) => buf.len(),
            Self::F32(buf) => buf.len(),
            Self::F64(buf) => buf.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.size_in_bytes() == 0
    }
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

/// How to interpret the contents of a tensor.
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

impl Tensor {
    #[inline]
    pub fn id(&self) -> TensorId {
        self.tensor_id
    }

    #[inline]
    pub fn shape(&self) -> &[TensorDimension] {
        self.shape.as_slice()
    }

    #[inline]
    pub fn num_dim(&self) -> usize {
        self.shape.len()
    }

    /// If this tensor is shaped as an image, return the height, width, and depth of it.
    pub fn image_height_width_depth(&self) -> Option<[u64; 3]> {
        if self.shape.len() == 2 {
            Some([self.shape[0].size, self.shape[1].size, 1])
        } else if self.shape.len() == 3 {
            let depth = self.shape[2].size;
            // gray, rgb, rgba
            if matches!(depth, 1 | 3 | 4) {
                Some([self.shape[0].size, self.shape[1].size, depth])
            } else {
                None
            }
        } else {
            None
        }
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

    #[inline]
    pub fn is_vector(&self) -> bool {
        let shape = &self.shape;
        shape.len() == 1 || { shape.len() == 2 && (shape[0].size == 1 || shape[1].size == 1) }
    }

    #[inline]
    pub fn meaning(&self) -> TensorDataMeaning {
        self.meaning
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

impl Component for Tensor {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.tensor".into()
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
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

    #[error(
        "tensors do not currently support f16 data (https://github.com/rerun-io/rerun/issues/854)"
    )]
    F16NotSupported,
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

tensor_type!(f32, F32);
tensor_type!(f64, F64);

// TODO(#854) Switch back to `tensor_type!` once we have F16 tensors
impl<'a> TryFrom<&'a Tensor> for ::ndarray::ArrayViewD<'a, half::f16> {
    type Error = TensorCastError;

    fn try_from(_: &'a Tensor) -> Result<Self, Self::Error> {
        Err(TensorCastError::F16NotSupported)
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "image")]
#[derive(thiserror::Error, Debug)]
pub enum TensorImageError {
    #[error(transparent)]
    Image(#[from] image::ImageError),

    #[error("Unsupported JPEG color type: {0:?}. Only RGB Jpegs are supported")]
    UnsupportedJpegColorType(image::ColorType),

    #[error("Unsupported color type: {0:?}. We support 8-bit, 16-bit, and f32 images, and RGB, RGBA, Luminance, and Luminance-Alpha.")]
    UnsupportedImageColorType(image::ColorType),

    #[error("Failed to load file: {0}")]
    ReadError(#[from] std::io::Error),
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
    /// Construct a tensor from the contents of a JPEG file on disk.
    ///
    /// Requires the `image` feature.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tensor_from_jpeg_file(
        image_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, TensorImageError> {
        let jpeg_bytes = std::fs::read(image_path)?;
        Self::tensor_from_jpeg_bytes(jpeg_bytes)
    }

    /// Construct a tensor from the contents of a JPEG file.
    ///
    /// Requires the `image` feature.
    pub fn tensor_from_jpeg_bytes(jpeg_bytes: Vec<u8>) -> Result<Self, TensorImageError> {
        use image::ImageDecoder as _;
        let jpeg = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&jpeg_bytes))?;
        if jpeg.color_type() != image::ColorType::Rgb8 {
            // TODO(emilk): support gray-scale jpeg as well
            return Err(TensorImageError::UnsupportedJpegColorType(
                jpeg.color_type(),
            ));
        }
        let (w, h) = jpeg.dimensions();

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

    /// Construct a tensor from something that can be turned into a [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_image(image: impl Into<image::DynamicImage>) -> Result<Self, TensorImageError> {
        Self::from_dynamic_image(image.into())
    }

    /// Construct a tensor from [`image::DynamicImage`].
    ///
    /// Requires the `image` feature.
    pub fn from_dynamic_image(image: image::DynamicImage) -> Result<Self, TensorImageError> {
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
                return Err(TensorImageError::UnsupportedImageColorType(image.color()));
            }
        };

        Ok(Self {
            tensor_id: TensorId::random(),
            shape: vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
                TensorDimension::depth(depth),
            ],
            data,
            meaning: TensorDataMeaning::Unknown,
            meter: None,
        })
    }
}

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
