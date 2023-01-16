use std::sync::Arc;

use half::f16;

use crate::{field_types, impl_into_enum, AnnotationContext, Mesh3D, ObjPath, ViewCoordinates};

pub use crate::field_types::{Pinhole, Rigid3, Transform};

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataType {
    // 1D:
    Bool,
    I32,
    F32,
    F64,
    Color,
    String,

    // ----------------------------
    // 2D:
    Vec2,
    BBox2D,

    // ----------------------------
    // 3D:
    Vec3,
    Box3,
    Mesh3D,
    Arrow3D,

    // ----------------------------
    // N-D:
    Tensor,

    /// A homogenous vector of data,
    /// represented by [`DataVec`]
    DataVec,

    // ----------------------------
    ObjPath,

    Transform,
    ViewCoordinates,
    AnnotationContext,
}

// ----------------------------------------------------------------------------

/// Marker traits for types we allow in the the data store.
///
/// Everything in [`data_types`] implement this, and nothing else.
pub trait DataTrait: 'static + Clone {
    fn data_typ() -> DataType;
}

pub mod data_types {
    use crate::field_types::Mesh3D;

    use super::DataTrait;
    use super::DataType;
    use super::Transform;

    impl DataTrait for super::DataVec {
        fn data_typ() -> DataType {
            DataType::DataVec
        }
    }

    impl DataTrait for bool {
        fn data_typ() -> DataType {
            DataType::Bool
        }
    }

    impl DataTrait for i32 {
        fn data_typ() -> DataType {
            DataType::I32
        }
    }

    impl DataTrait for f32 {
        fn data_typ() -> DataType {
            DataType::F32
        }
    }

    impl DataTrait for f64 {
        fn data_typ() -> DataType {
            DataType::F64
        }
    }

    // TODO(emilk): consider using `Arc<str>` or similar instead, for faster cloning.
    impl DataTrait for String {
        fn data_typ() -> DataType {
            DataType::String
        }
    }

    /// RGBA unmultiplied/separate alpha
    pub type Color = [u8; 4];
    impl DataTrait for Color {
        fn data_typ() -> DataType {
            DataType::Color
        }
    }

    // ---

    pub type Vec2 = [f32; 2];
    impl DataTrait for Vec2 {
        fn data_typ() -> DataType {
            DataType::Vec2
        }
    }

    impl DataTrait for crate::BBox2D {
        fn data_typ() -> DataType {
            DataType::BBox2D
        }
    }

    impl DataTrait for crate::ClassicTensor {
        fn data_typ() -> DataType {
            DataType::Tensor
        }
    }

    // ---

    pub type Vec3 = [f32; 3];
    impl DataTrait for Vec3 {
        fn data_typ() -> DataType {
            DataType::Vec3
        }
    }

    impl DataTrait for crate::Box3 {
        fn data_typ() -> DataType {
            DataType::Box3
        }
    }

    impl DataTrait for Mesh3D {
        fn data_typ() -> DataType {
            DataType::Mesh3D
        }
    }

    impl DataTrait for crate::Arrow3D {
        fn data_typ() -> DataType {
            DataType::Arrow3D
        }
    }

    // ---

    impl DataTrait for crate::ObjPath {
        fn data_typ() -> DataType {
            DataType::ObjPath
        }
    }

    impl DataTrait for Transform {
        fn data_typ() -> DataType {
            DataType::Transform
        }
    }

    impl DataTrait for crate::ViewCoordinates {
        fn data_typ() -> DataType {
            DataType::ViewCoordinates
        }
    }

    impl DataTrait for crate::AnnotationContext {
        fn data_typ() -> DataType {
            DataType::AnnotationContext
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Data {
    // 1D:
    Bool(bool),
    I32(i32),
    F32(f32),
    F64(f64),
    Color(data_types::Color),
    String(String),

    // ----------------------------
    // 2D:
    Vec2(data_types::Vec2),
    BBox2D(BBox2D),

    // ----------------------------
    // 3D:
    Vec3(data_types::Vec3),
    Box3(Box3),
    Mesh3D(Mesh3D),
    Arrow3D(Arrow3D),

    // ----------------------------
    // N-D:
    Tensor(ClassicTensor),

    /// Homogenous vector
    DataVec(DataVec),

    // ----------------------------
    // Meta:
    /// One object referring to another (a pointer).
    ObjPath(ObjPath),

    Transform(crate::field_types::Transform),
    ViewCoordinates(ViewCoordinates),
    AnnotationContext(AnnotationContext),
}

impl Data {
    #[inline]
    pub fn data_type(&self) -> DataType {
        match self {
            Self::Bool(_) => DataType::Bool,
            Self::I32(_) => DataType::I32,
            Self::F32(_) => DataType::F32,
            Self::F64(_) => DataType::F64,
            Self::Color(_) => DataType::Color,
            Self::String(_) => DataType::String,

            Self::Vec2(_) => DataType::Vec2,
            Self::BBox2D(_) => DataType::BBox2D,

            Self::Vec3(_) => DataType::Vec3,
            Self::Box3(_) => DataType::Box3,
            Self::Mesh3D(_) => DataType::Mesh3D,
            Self::Arrow3D(_) => DataType::Arrow3D,

            Self::Tensor(_) => DataType::Tensor,
            Self::DataVec(_) => DataType::DataVec,

            Self::ObjPath(_) => DataType::ObjPath,

            Self::Transform(_) => DataType::Transform,
            Self::ViewCoordinates(_) => DataType::ViewCoordinates,
            Self::AnnotationContext(_) => DataType::AnnotationContext,
        }
    }
}

impl_into_enum!(bool, Data, Bool);
impl_into_enum!(i32, Data, I32);
impl_into_enum!(f32, Data, F32);
impl_into_enum!(f64, Data, F64);
impl_into_enum!(BBox2D, Data, BBox2D);
impl_into_enum!(ClassicTensor, Data, Tensor);
impl_into_enum!(Box3, Data, Box3);
impl_into_enum!(Mesh3D, Data, Mesh3D);
impl_into_enum!(ObjPath, Data, ObjPath);
impl_into_enum!(Transform, Data, Transform);
impl_into_enum!(ViewCoordinates, Data, ViewCoordinates);
impl_into_enum!(AnnotationContext, Data, AnnotationContext);

// ----------------------------------------------------------------------------

/// Vectorized, type-erased version of [`Data`].
// TODO(emilk): we should generalize this to a tensor.
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataVec {
    Bool(Vec<bool>),
    I32(Vec<i32>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    Color(Vec<data_types::Color>),
    String(Vec<String>),

    Vec2(Vec<data_types::Vec2>),
    BBox2D(Vec<BBox2D>),

    Vec3(Vec<data_types::Vec3>),
    Box3(Vec<Box3>),
    Mesh3D(Vec<Mesh3D>),
    Arrow3D(Vec<Arrow3D>),

    Tensor(Vec<ClassicTensor>),

    /// A vector of [`DataVec`] (vector of vectors)
    DataVec(Vec<DataVec>),

    ObjPath(Vec<ObjPath>),

    Transform(Vec<Transform>),
    ViewCoordinates(Vec<ViewCoordinates>),
    AnnotationContext(Vec<AnnotationContext>),
}

/// Do the same thing with all members of a [`Data`].
///
/// ```
/// # use re_log_types::{Data, data_map};
/// # let data: Data = Data::F32(0.0);
/// data_map!(data, |data| { dbg!(data); });
/// ```
#[macro_export]
macro_rules! data_map(
    ($data: expr, |$value: pat_param| $action: expr) => ({
        match $data {
            $crate::Data::Bool($value) => $action,
            $crate::Data::I32($value) => $action,
            $crate::Data::F32($value) => $action,
            $crate::Data::F64($value) => $action,
            $crate::Data::Color($value) => $action,
            $crate::Data::String($value) => $action,
            $crate::Data::Vec2($value) => $action,
            $crate::Data::BBox2D($value) => $action,
            $crate::Data::Vec3($value) => $action,
            $crate::Data::Box3($value) => $action,
            $crate::Data::Mesh3D($value) => $action,
            $crate::Data::Arrow3D($value) => $action,
            $crate::Data::Tensor($value) => $action,
            $crate::Data::DataVec($value) => $action,
            $crate::Data::ObjPath($value) => $action,
            $crate::Data::Transform($value) => $action,
            $crate::Data::ViewCoordinates($value) => $action,
            $crate::Data::AnnotationContext($value) => $action,
        }
    });
);

/// Map a [`DataType`] to the correct instance of `Option::<T>::None`.
///
/// ```
/// # use re_log_types::{DataType, data_type_map_none};
/// # let data_type: DataType = DataType::F32;
/// data_type_map_none!(data_type, |data_none| { assert!(data_none.is_none()); });
/// ```
#[macro_export]
macro_rules! data_type_map_none(
    ($data_type: expr, |$value: pat_param| $action: expr) => ({
        match $data_type {
            $crate::DataType::Bool => {
                let $value = Option::<bool>::None;
                 $action
            },
            $crate::DataType::I32 => {
                let $value = Option::<i32>::None;
                $action
            }
            $crate::DataType::F32 => {
                let $value = Option::<f32>::None;
                $action
            },
            $crate::DataType::F64 => {
                let $value = Option::<f64>::None;
                $action
            },
            $crate::DataType::Color => {
                let $value = Option::<$crate::data_types::Color>::None;
                $action
            },
            $crate::DataType::String => {
                let $value = Option::<String>::None;
                $action
            },
            $crate::DataType::Vec2 => {
                let $value = Option::<$crate::data_types::Vec2>::None;
                $action
            },
            $crate::DataType::BBox2D => {
                let $value = Option::<$crate::BBox2D>::None;
                $action
            },
            $crate::DataType::Vec3 => {
                let $value = Option::<$crate::data_types::Vec3>::None;
                $action
            },
            $crate::DataType::Box3 => {
                let $value = Option::<$crate::Box3>::None;
                $action
            },
            $crate::DataType::Mesh3D => {
                let $value = Option::<$crate::Mesh3D>::None;
                $action
            },
            $crate::DataType::Arrow3D => {
                let $value = Option::<$crate::Arrow3D>::None;
                $action
            },
            $crate::DataType::Tensor => {
                let $value = Option::<$crate::ClassicTensor>::None;
                $action
            },
            $crate::DataType::DataVec => {
                let $value = Option::<$crate::DataVec>::None;
                $action
            },
            $crate::DataType::ObjPath => {
                let $value = Option::<$crate::ObjPath>::None;
                $action
            },
            $crate::DataType::Transform => {
                let $value = Option::<$crate::field_types::Transform>::None;
                $action
            },
            $crate::DataType::ViewCoordinates => {
                let $value = Option::<$crate::ViewCoordinates>::None;
                $action
            },
            $crate::DataType::AnnotationContext => {
                let $value = Option::<$crate::AnnotationContext>::None;
                $action
            },
        }
    });
);

/// Do the same thing with all members of a [`DataVec`].
///
/// ```
/// # use re_log_types::{DataVec, data_vec_map};
/// # let data_vec: DataVec = DataVec::F32(vec![]);
/// let length = data_vec_map!(data_vec, |vec| vec.len());
/// ```
#[macro_export]
macro_rules! data_vec_map(
    ($data_vec: expr, |$vec: pat_param| $action: expr) => ({
        match $data_vec {
            $crate::DataVec::Bool($vec) => $action,
            $crate::DataVec::I32($vec) => $action,
            $crate::DataVec::F32($vec) => $action,
            $crate::DataVec::F64($vec) => $action,
            $crate::DataVec::Color($vec) => $action,
            $crate::DataVec::String($vec) => $action,
            $crate::DataVec::Vec2($vec) => $action,
            $crate::DataVec::BBox2D($vec) => $action,
            $crate::DataVec::Vec3($vec) => $action,
            $crate::DataVec::Box3($vec) => $action,
            $crate::DataVec::Mesh3D($vec) => $action,
            $crate::DataVec::Arrow3D($vec) => $action,
            $crate::DataVec::Tensor($vec) => $action,
            $crate::DataVec::DataVec($vec) => $action,
            $crate::DataVec::ObjPath($vec) => $action,
            $crate::DataVec::Transform($vec) => $action,
            $crate::DataVec::ViewCoordinates($vec) => $action,
            $crate::DataVec::AnnotationContext($vec) => $action,
        }
    });
);

impl DataVec {
    #[inline]
    pub fn element_data_type(&self) -> DataType {
        match self {
            Self::Bool(_) => DataType::Bool,
            Self::I32(_) => DataType::I32,
            Self::F32(_) => DataType::F32,
            Self::F64(_) => DataType::F64,
            Self::Color(_) => DataType::Color,
            Self::String(_) => DataType::String,

            Self::Vec2(_) => DataType::Vec2,
            Self::BBox2D(_) => DataType::BBox2D,

            Self::Vec3(_) => DataType::Vec3,
            Self::Box3(_) => DataType::Box3,
            Self::Mesh3D(_) => DataType::Mesh3D,
            Self::Arrow3D(_) => DataType::Arrow3D,

            Self::Tensor(_) => DataType::Tensor,
            Self::DataVec(_) => DataType::DataVec,

            Self::ObjPath(_) => DataType::ObjPath,

            Self::Transform(_) => DataType::Transform,
            Self::ViewCoordinates(_) => DataType::ViewCoordinates,
            Self::AnnotationContext(_) => DataType::AnnotationContext,
        }
    }

    pub fn empty_from_data_type(data_type: DataType) -> Self {
        match data_type {
            DataType::Bool => Self::Bool(vec![]),
            DataType::I32 => Self::I32(vec![]),
            DataType::F32 => Self::F32(vec![]),
            DataType::F64 => Self::F64(vec![]),
            DataType::Color => Self::Color(vec![]),
            DataType::String => Self::String(vec![]),

            DataType::Vec2 => Self::Vec2(vec![]),
            DataType::BBox2D => Self::BBox2D(vec![]),

            DataType::Vec3 => Self::Vec3(vec![]),
            DataType::Box3 => Self::Box3(vec![]),
            DataType::Mesh3D => Self::Mesh3D(vec![]),
            DataType::Arrow3D => Self::Arrow3D(vec![]),

            DataType::Tensor => Self::Tensor(vec![]),
            DataType::DataVec => Self::DataVec(vec![]),

            DataType::ObjPath => Self::ObjPath(vec![]),

            DataType::Transform => Self::Transform(vec![]),
            DataType::ViewCoordinates => Self::ViewCoordinates(vec![]),
            DataType::AnnotationContext => Self::AnnotationContext(vec![]),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        data_vec_map!(self, |vec| vec.len())
    }

    pub fn last(&self) -> Option<Data> {
        match self {
            Self::Bool(vec) => vec.last().cloned().map(Data::Bool),
            Self::I32(vec) => vec.last().cloned().map(Data::I32),
            Self::F32(vec) => vec.last().cloned().map(Data::F32),
            Self::F64(vec) => vec.last().cloned().map(Data::F64),
            Self::Color(vec) => vec.last().cloned().map(Data::Color),
            Self::String(vec) => vec.last().cloned().map(Data::String),

            Self::Vec2(vec) => vec.last().cloned().map(Data::Vec2),
            Self::BBox2D(vec) => vec.last().cloned().map(Data::BBox2D),

            Self::Vec3(vec) => vec.last().cloned().map(Data::Vec3),
            Self::Box3(vec) => vec.last().cloned().map(Data::Box3),
            Self::Mesh3D(vec) => vec.last().cloned().map(Data::Mesh3D),
            Self::Arrow3D(vec) => vec.last().cloned().map(Data::Arrow3D),

            Self::Tensor(vec) => vec.last().cloned().map(Data::Tensor),
            Self::DataVec(vec) => vec.last().cloned().map(Data::DataVec),

            Self::ObjPath(vec) => vec.last().cloned().map(Data::ObjPath),

            Self::Transform(vec) => vec.last().cloned().map(Data::Transform),
            Self::ViewCoordinates(vec) => vec.last().cloned().map(Data::ViewCoordinates),
            Self::AnnotationContext(vec) => vec.last().cloned().map(Data::AnnotationContext),
        }
    }

    pub fn as_vec_of_vec2(&self, what: &str) -> Option<&[[f32; 2]]> {
        if let DataVec::Vec2(vec) = self {
            Some(vec)
        } else {
            re_log::warn_once!(
                "Expected {what} to be Vec<Vec2>, got Vec<{:?}>",
                self.element_data_type()
            );
            None
        }
    }

    pub fn as_vec_of_vec3(&self, what: &str) -> Option<&[[f32; 3]]> {
        if let DataVec::Vec3(vec) = self {
            Some(vec)
        } else {
            re_log::warn_once!(
                "Expected {what} to be Vec<Vec3>, got Vec<{:?}>",
                self.element_data_type()
            );
            None
        }
    }
}

impl std::fmt::Debug for DataVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataVec")
            .field("len", &self.len())
            .field("data_type", &self.element_data_type())
            .finish_non_exhaustive()
    }
}

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

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Arrow3D {
    pub origin: [f32; 3],
    pub vector: [f32; 3],
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
