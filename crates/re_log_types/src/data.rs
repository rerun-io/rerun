use crate::{impl_into_enum, ObjPath};

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataType {
    // 1D:
    I32,
    F32,
    Color,
    String,

    // ----------------------------
    // 2D:
    Vec2,
    BBox2D,
    LineSegments2D,

    // ----------------------------
    // 3D:
    Vec3,
    Box3,
    Path3D,
    LineSegments3D,
    Mesh3D,
    Camera,

    // ----------------------------
    // N-D:
    Tensor,

    // ----------------------------
    Space,
}

// ----------------------------------------------------------------------------

/// Marker traits for types we allow in the the data store.
///
/// Everything in [`data_types`] implement this, and nothing else.
pub trait DataTrait: 'static + Clone {
    fn data_typ() -> DataType;
}

pub mod data_types {
    use super::DataTrait;
    use super::DataType;

    /// For batches
    impl<T: DataTrait> DataTrait for Vec<T> {
        fn data_typ() -> DataType {
            T::data_typ()
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

    pub type LineSegments2D = Vec<LineSegment2D>;
    pub type LineSegment2D = [Vec2; 2];
    impl DataTrait for LineSegment2D {
        fn data_typ() -> DataType {
            DataType::LineSegments2D
        }
    }
    impl DataTrait for crate::Tensor {
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

    pub type Path3D = Vec<Vec3>;

    pub type LineSegments3D = Vec<LineSegment3D>;
    pub type LineSegment3D = [Vec3; 2];
    impl DataTrait for LineSegment3D {
        fn data_typ() -> DataType {
            DataType::LineSegments3D
        }
    }

    impl DataTrait for crate::Mesh3D {
        fn data_typ() -> DataType {
            DataType::Mesh3D
        }
    }

    impl DataTrait for crate::Camera {
        fn data_typ() -> DataType {
            DataType::Camera
        }
    }

    // ---

    impl DataTrait for crate::ObjPath {
        fn data_typ() -> DataType {
            DataType::Space
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Data {
    // 1D:
    I32(i32),
    F32(f32),
    Color(data_types::Color),
    String(String),

    // ----------------------------
    // 2D:
    Vec2(data_types::Vec2),
    BBox2D(BBox2D),
    LineSegments2D(data_types::LineSegments2D),

    // ----------------------------
    // 3D:
    Vec3(data_types::Vec3),
    Box3(Box3),
    Path3D(data_types::Path3D),
    LineSegments3D(data_types::LineSegments3D),
    Mesh3D(Mesh3D),
    Camera(Camera),

    // ----------------------------
    // N-D:
    Tensor(Tensor),

    // ----------------------------
    // Meta:
    /// Used for specifying which space data belongs to.
    Space(ObjPath),
}

impl Data {
    #[inline]
    pub fn data_type(&self) -> DataType {
        match self {
            Self::I32(_) => DataType::I32,
            Self::F32(_) => DataType::F32,
            Self::Color(_) => DataType::Color,
            Self::String(_) => DataType::String,

            Self::Vec2(_) => DataType::Vec2,
            Self::BBox2D(_) => DataType::BBox2D,
            Self::LineSegments2D(_) => DataType::LineSegments2D,

            Self::Vec3(_) => DataType::Vec3,
            Self::Box3(_) => DataType::Box3,
            Self::Path3D(_) => DataType::Path3D,
            Self::LineSegments3D(_) => DataType::LineSegments3D,
            Self::Mesh3D(_) => DataType::Mesh3D,
            Self::Camera(_) => DataType::Camera,

            Self::Tensor(_) => DataType::Tensor,

            Self::Space(_) => DataType::Space,
        }
    }
}

impl_into_enum!(i32, Data, I32);
impl_into_enum!(f32, Data, F32);
impl_into_enum!(BBox2D, Data, BBox2D);
impl_into_enum!(Tensor, Data, Tensor);
impl_into_enum!(Box3, Data, Box3);
impl_into_enum!(Mesh3D, Data, Mesh3D);
impl_into_enum!(Camera, Data, Camera);
impl_into_enum!(ObjPath, Data, Space);

// ----------------------------------------------------------------------------

/// Vectorized, type-erased version of [`Data`].
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataVec {
    I32(Vec<i32>),
    F32(Vec<f32>),
    Color(Vec<data_types::Color>),
    String(Vec<String>),

    Vec2(Vec<data_types::Vec2>),
    BBox2D(Vec<BBox2D>),
    LineSegments2D(Vec<data_types::LineSegments2D>),

    Vec3(Vec<data_types::Vec3>),
    Box3(Vec<Box3>),
    Path3D(Vec<data_types::Path3D>),
    LineSegments3D(Vec<data_types::LineSegments3D>),
    Mesh3D(Vec<Mesh3D>),
    Camera(Vec<Camera>),

    Tensor(Vec<Tensor>),

    Space(Vec<ObjPath>),
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
            Data::I32($value) => $action,
            Data::F32($value) => $action,
            Data::Color($value) => $action,
            Data::String($value) => $action,
            Data::Vec2($value) => $action,
            Data::BBox2D($value) => $action,
            Data::LineSegments2D($value) => $action,
            Data::Vec3($value) => $action,
            Data::Box3($value) => $action,
            Data::Path3D($value) => $action,
            Data::LineSegments3D($value) => $action,
            Data::Mesh3D($value) => $action,
            Data::Camera($value) => $action,
            Data::Tensor($value) => $action,
            Data::Space($value) => $action,
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
            DataVec::I32($vec) => $action,
            DataVec::F32($vec) => $action,
            DataVec::Color($vec) => $action,
            DataVec::String($vec) => $action,
            DataVec::Vec2($vec) => $action,
            DataVec::BBox2D($vec) => $action,
            DataVec::LineSegments2D($vec) => $action,
            DataVec::Tensor($vec) => $action,
            DataVec::Vec3($vec) => $action,
            DataVec::Box3($vec) => $action,
            DataVec::Path3D($vec) => $action,
            DataVec::LineSegments3D($vec) => $action,
            DataVec::Mesh3D($vec) => $action,
            DataVec::Camera($vec) => $action,
            DataVec::Space($vec) => $action,
        }
    });
);

impl DataVec {
    #[inline]
    pub fn data_type(&self) -> DataType {
        match self {
            Self::I32(_) => DataType::I32,
            Self::F32(_) => DataType::F32,
            Self::Color(_) => DataType::Color,
            Self::String(_) => DataType::String,

            Self::Vec2(_) => DataType::Vec2,
            Self::BBox2D(_) => DataType::BBox2D,
            Self::LineSegments2D(_) => DataType::LineSegments2D,

            Self::Vec3(_) => DataType::Vec3,
            Self::Box3(_) => DataType::Box3,
            Self::Path3D(_) => DataType::Path3D,
            Self::LineSegments3D(_) => DataType::LineSegments3D,
            Self::Mesh3D(_) => DataType::Mesh3D,
            Self::Camera(_) => DataType::Camera,

            Self::Tensor(_) => DataType::Tensor,

            Self::Space(_) => DataType::Space,
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
            Self::I32(vec) => vec.last().cloned().map(Data::I32),
            Self::F32(vec) => vec.last().cloned().map(Data::F32),
            Self::Color(vec) => vec.last().cloned().map(Data::Color),
            Self::String(vec) => vec.last().cloned().map(Data::String),

            Self::Vec2(vec) => vec.last().cloned().map(Data::Vec2),
            Self::BBox2D(vec) => vec.last().cloned().map(Data::BBox2D),
            Self::LineSegments2D(vec) => vec.last().cloned().map(Data::LineSegments2D),

            Self::Vec3(vec) => vec.last().cloned().map(Data::Vec3),
            Self::Box3(vec) => vec.last().cloned().map(Data::Box3),
            Self::Path3D(vec) => vec.last().cloned().map(Data::Path3D),
            Self::LineSegments3D(vec) => vec.last().cloned().map(Data::LineSegments3D),
            Self::Mesh3D(vec) => vec.last().cloned().map(Data::Mesh3D),
            Self::Camera(vec) => vec.last().cloned().map(Data::Camera),

            Self::Tensor(vec) => vec.last().cloned().map(Data::Tensor),

            Self::Space(vec) => vec.last().cloned().map(Data::Space),
        }
    }
}

impl std::fmt::Debug for DataVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataVec")
            .field("data_type", &self.data_type())
            .field("len", &self.len())
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

/// Order: XYZW
pub type Quaternion = [f32; 4];

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Camera {
    /// How is the camera rotated, compared to the parent space?
    ///
    /// World from local.
    pub rotation: Quaternion,

    /// Where is the camera?
    pub position: [f32; 3],

    /// Column-major intrinsics matrix for projecting to pixel coordinates.
    ///
    /// Example:
    /// ```text
    /// [[1496.1, 0.0,    0.0], // col 0
    ///  [0.0,    1496.1, 0.0], // col 1
    ///  [980.5,  744.5,  1.0]] // col 2
    /// ```
    pub intrinsics: Option<[[f32; 3]; 3]>,

    /// Pixel resolution (usually integers). Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    pub resolution: Option<[f32; 2]>,
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Mesh3D {
    Encoded(EncodedMesh3D),
    Raw(RawMesh3D),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RawMesh3D {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
}

/// Compressed/encoded mesh format
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EncodedMesh3D {
    pub format: MeshFormat,
    pub bytes: std::sync::Arc<[u8]>,
    /// four columns of a transformation matrix
    pub transform: [[f32; 4]; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum MeshFormat {
    Gltf,
    Glb,
    Obj,
}

// ----------------------------------------------------------------------------

/// The data types supported by a [`Tensor`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TensorDataType {
    /// Commonly used for sRGB(A)
    U8,

    /// Some depth images and some high-bitrate images
    U16,

    /// Commonly used for depth images
    F32,
}

/// The data types supported by a [`Tensor`].
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TensorData {
    /// Densely packed tensor
    Dense(Vec<u8>),

    /// A JPEG image.
    ///
    /// This can only represent tensors with [`DataType::U8`]
    /// of dimensions `[h, w, 3]` (RGB) or `[h, w]` (grayscale).
    Jpeg(Vec<u8>),
}

impl std::fmt::Debug for TensorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TensorData::Dense(bytes) => {
                f.write_fmt(format_args!("TensorData::Dense({} bytes)", bytes.len()))
            }
            TensorData::Jpeg(bytes) => {
                f.write_fmt(format_args!("TensorData::Jpeg({} bytes)", bytes.len()))
            }
        }
    }
}

/// An N-dimensional colelction of numbers.
///
/// Most often used to describe image pixels.
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Tensor {
    /// Example: `[h, w, 3]` for an RGB image, stored in row-major-order.
    /// The order martches that of numpy etc, and is ordered so that
    /// the "tighest wound" dimension is last.
    ///
    /// Conceptually `[h,w]` == `[h,w,1]` == `[h,w,1,1,1]` etc.
    pub shape: Vec<u64>,

    /// The per-element data format.
    /// numpy calls this `dtype`.
    pub dtype: TensorDataType,

    /// The actual contents of the tensor
    pub data: TensorData,
}

impl std::fmt::Debug for Tensor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tensor")
            .field("shape", &self.shape)
            .field("dtype", &self.dtype)
            .field("data", &self.data)
            .finish()
    }
}
