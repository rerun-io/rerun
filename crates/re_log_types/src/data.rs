use std::sync::Arc;

use crate::{impl_into_enum, ObjPath};

use self::data_types::Vec3;

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataType {
    // 1D:
    Bool,
    I32,
    F32,
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
    Camera,

    // ----------------------------
    // N-D:
    Tensor,

    /// A homogenous vector of data,
    /// represented by [`DataVec`]
    DataVec,

    // ----------------------------
    ObjPath,

    Transform,
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
            DataType::ObjPath
        }
    }

    impl DataTrait for crate::Transform {
        fn data_typ() -> DataType {
            DataType::Transform
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
    Camera(Camera),

    // ----------------------------
    // N-D:
    Tensor(Tensor),

    /// Homogenous vector
    DataVec(DataVec),

    // ----------------------------
    // Meta:
    /// One object referring to another (a pointer).
    ObjPath(ObjPath),

    Transform(Transform),
}

impl Data {
    #[inline]
    pub fn data_type(&self) -> DataType {
        match self {
            Self::Bool(_) => DataType::Bool,
            Self::I32(_) => DataType::I32,
            Self::F32(_) => DataType::F32,
            Self::Color(_) => DataType::Color,
            Self::String(_) => DataType::String,

            Self::Vec2(_) => DataType::Vec2,
            Self::BBox2D(_) => DataType::BBox2D,

            Self::Vec3(_) => DataType::Vec3,
            Self::Box3(_) => DataType::Box3,
            Self::Mesh3D(_) => DataType::Mesh3D,
            Self::Camera(_) => DataType::Camera,

            Self::Tensor(_) => DataType::Tensor,
            Self::DataVec(_) => DataType::DataVec,

            Self::ObjPath(_) => DataType::ObjPath,

            Self::Transform(_) => DataType::Transform,
        }
    }
}

impl_into_enum!(bool, Data, Bool);
impl_into_enum!(i32, Data, I32);
impl_into_enum!(f32, Data, F32);
impl_into_enum!(BBox2D, Data, BBox2D);
impl_into_enum!(Tensor, Data, Tensor);
impl_into_enum!(Box3, Data, Box3);
impl_into_enum!(Mesh3D, Data, Mesh3D);
impl_into_enum!(Camera, Data, Camera);
impl_into_enum!(ObjPath, Data, ObjPath);
impl_into_enum!(Transform, Data, Transform);

// ----------------------------------------------------------------------------

/// Vectorized, type-erased version of [`Data`].
// TODO(emilk): we should generalize this to a tensor.
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataVec {
    Bool(Vec<bool>),
    I32(Vec<i32>),
    F32(Vec<f32>),
    Color(Vec<data_types::Color>),
    String(Vec<String>),

    Vec2(Vec<data_types::Vec2>),
    BBox2D(Vec<BBox2D>),

    Vec3(Vec<data_types::Vec3>),
    Box3(Vec<Box3>),
    Mesh3D(Vec<Mesh3D>),
    Camera(Vec<Camera>),

    Tensor(Vec<Tensor>),

    /// A vector of [`DataVec`] (vector of vectors)
    DataVec(Vec<DataVec>),

    ObjPath(Vec<ObjPath>),

    Transform(Vec<Transform>),
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
            $crate::Data::Color($value) => $action,
            $crate::Data::String($value) => $action,
            $crate::Data::Vec2($value) => $action,
            $crate::Data::BBox2D($value) => $action,
            $crate::Data::Vec3($value) => $action,
            $crate::Data::Box3($value) => $action,
            $crate::Data::Mesh3D($value) => $action,
            $crate::Data::Camera($value) => $action,
            $crate::Data::Tensor($value) => $action,
            $crate::Data::DataVec($value) => $action,
            $crate::Data::ObjPath($value) => $action,
            $crate::Data::Transform($value) => $action,
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
            $crate::DataVec::Color($vec) => $action,
            $crate::DataVec::String($vec) => $action,
            $crate::DataVec::Vec2($vec) => $action,
            $crate::DataVec::BBox2D($vec) => $action,
            $crate::DataVec::Vec3($vec) => $action,
            $crate::DataVec::Box3($vec) => $action,
            $crate::DataVec::Mesh3D($vec) => $action,
            $crate::DataVec::Camera($vec) => $action,
            $crate::DataVec::Tensor($vec) => $action,
            $crate::DataVec::DataVec($vec) => $action,
            $crate::DataVec::ObjPath($vec) => $action,
            $crate::DataVec::Transform($vec) => $action,
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
            Self::Color(_) => DataType::Color,
            Self::String(_) => DataType::String,

            Self::Vec2(_) => DataType::Vec2,
            Self::BBox2D(_) => DataType::BBox2D,

            Self::Vec3(_) => DataType::Vec3,
            Self::Box3(_) => DataType::Box3,
            Self::Mesh3D(_) => DataType::Mesh3D,
            Self::Camera(_) => DataType::Camera,

            Self::Tensor(_) => DataType::Tensor,
            Self::DataVec(_) => DataType::DataVec,

            Self::ObjPath(_) => DataType::ObjPath,

            Self::Transform(_) => DataType::Transform,
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
            Self::Color(vec) => vec.last().cloned().map(Data::Color),
            Self::String(vec) => vec.last().cloned().map(Data::String),

            Self::Vec2(vec) => vec.last().cloned().map(Data::Vec2),
            Self::BBox2D(vec) => vec.last().cloned().map(Data::BBox2D),

            Self::Vec3(vec) => vec.last().cloned().map(Data::Vec3),
            Self::Box3(vec) => vec.last().cloned().map(Data::Box3),
            Self::Mesh3D(vec) => vec.last().cloned().map(Data::Mesh3D),
            Self::Camera(vec) => vec.last().cloned().map(Data::Camera),

            Self::Tensor(vec) => vec.last().cloned().map(Data::Tensor),
            Self::DataVec(vec) => vec.last().cloned().map(Data::DataVec),

            Self::ObjPath(vec) => vec.last().cloned().map(Data::ObjPath),

            Self::Transform(vec) => vec.last().cloned().map(Data::Transform),
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

/// Order: XYZW
pub type Quaternion = [f32; 4];

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Camera {
    pub extrinsics: Extrinsics,
    pub intrinsics: Option<Intrinsics>,

    /// The 2D space that this camera projects into.
    pub target_space: Option<ObjPath>,
}

/// Camera pose
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Extrinsics {
    /// How is the camera rotated?
    ///
    /// This transforms world-space from camera-space.
    ///
    /// The exact meaning of this depends on [`Self::camera_space_convention`].
    /// For instance, using [`CameraSpaceConvention::XRightYDownZFwd`],
    /// [`Self::rotation`] rotates the +Z axis so that it points in the direction
    /// the camera is facing in world space.
    pub rotation: Quaternion,

    /// Where is the camera in world space?
    pub position: [f32; 3],

    /// What is the users camera-space coordinate system?
    pub camera_space_convention: CameraSpaceConvention,
}

/// Camera projection
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Intrinsics {
    /// Column-major intrinsics matrix.
    /// Image coordinates from view coordinates (via projection).
    ///
    /// Example:
    /// ```text
    /// [[1496.1, 0.0,    0.0], // col 0
    ///  [0.0,    1496.1, 0.0], // col 1
    ///  [980.5,  744.5,  1.0]] // col 2
    /// ```
    pub intrinsics_matrix: [[f32; 3]; 3],

    /// Pixel resolution (usually integers). Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    pub resolution: [f32; 2],
}

impl Intrinsics {
    /// Field of View on the Y axis, i.e. the angle between top and bottom.
    pub fn fov_y(&self) -> f32 {
        2.0 * (0.5 * self.resolution[1] / self.intrinsics_matrix[1][1]).atan()
    }
}

/// Convention for the coordinate system of the camera.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CameraSpaceConvention {
    /// Right-handed system used by ARKit and PyTorch3D.
    /// * +X=right
    /// * +Y=up
    /// * +Z=back (camera looks long -Z)
    XRightYUpZBack,

    /// Right-handed system used by OpenCV.
    /// * +X=right
    /// * +Y=down
    /// * +Z=forward
    XRightYDownZFwd,
}

impl CameraSpaceConvention {
    /// Rerun uses the view-space convention of +X=right, +Y=up, -Z=fwd.
    ///
    /// This returns the direction of the X,Y,Z axis in the Rerun convention.
    ///
    /// Another way of looking at this is that it returns the columns in
    /// the matrix that transforms one convention to the other.
    pub fn axis_dirs_in_rerun_view_space(&self) -> [Vec3; 3] {
        match self {
            Self::XRightYUpZBack => {
                [
                    [1.0, 0.0, 0.0], //
                    [0.0, 1.0, 0.0], //
                    [0.0, 0.0, 1.0], //
                ]
            }
            Self::XRightYDownZFwd => {
                [
                    [1.0, 0.0, 0.0],  //
                    [0.0, -1.0, 0.0], //
                    [0.0, 0.0, -1.0], //
                ]
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// A transform between two spaces.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Transform {
    /// The parent is a 3D space, the child a camera space.
    Extrinsics(Extrinsics),

    /// The parent is some local camera space, the child an image space.
    Intrinsics(Intrinsics),
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Mesh3D {
    Encoded(EncodedMesh3D),
    Raw(Arc<RawMesh3D>),
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
    /// four columns of an affine transformation matrix
    pub transform: [[f32; 3]; 4],
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TensorDataType {
    /// Commonly used for sRGB(A)
    U8,

    /// Some depth images and some high-bitrate images
    U16,

    /// Commonly used for depth images
    F32,
}

impl TensorDataType {
    /// Number of bytes used by the type
    #[inline]
    pub fn size(&self) -> u64 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::F32 => 4,
        }
    }
}

pub trait TensorDataTypeTrait: Copy + Clone + Send + Sync {
    const DTYPE: TensorDataType;
}
impl TensorDataTypeTrait for u8 {
    const DTYPE: TensorDataType = TensorDataType::U8;
}
impl TensorDataTypeTrait for u16 {
    const DTYPE: TensorDataType = TensorDataType::U16;
}
impl TensorDataTypeTrait for f32 {
    const DTYPE: TensorDataType = TensorDataType::F32;
}

/// The data that can be stored in a [`Tensor`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TensorElement {
    /// Commonly used for sRGB(A)
    U8(u8),

    /// Some depth images and some high-bitrate images
    U16(u16),

    /// Commonly used for depth images
    F32(f32),
}

impl TensorElement {
    #[inline]
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::U8(value) => *value as _,
            Self::U16(value) => *value as _,
            Self::F32(value) => *value as _,
        }
    }
}

/// The data types supported by a [`Tensor`].
///
/// NOTE: `PartialEq` takes into account _how_ the data is stored,
/// which can be surprising! As of 2022-08-15, `PartialEq` is only used by tests.
///
/// [`TensorDataStore`] uses [`Arc`] internally so that cloning a [`Tensor`] is cheap
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

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TensorDimension {
    /// Number of elements on this dimension.
    /// I.e. size-1 is the maximum allowed index.
    pub size: u64,

    /// Optional name of the dimension, e.g. "color" or "width"
    pub name: String,
}

impl TensorDimension {
    const DEFAULT_NAME_WIDTH: &'static str = "width";
    const DEFAULT_NAME_HEIGHT: &'static str = "height";
    const DEFAULT_NAME_DEPTH: &'static str = "depth";

    pub fn height(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_HEIGHT))
    }

    pub fn width(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_WIDTH))
    }

    pub fn depth(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_DEPTH))
    }

    pub fn named(size: u64, name: String) -> Self {
        Self { size, name }
    }

    pub fn unnamed(size: u64) -> Self {
        Self {
            size,
            name: String::new(),
        }
    }
}

impl std::fmt::Debug for TensorDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            self.size.fmt(f)
        } else {
            write!(f, "{}={}", self.name, self.size)
        }
    }
}

/// An N-dimensional collection of numbers.
///
/// Most often used to describe image pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Tensor {
    /// Example: `[h, w, 3]` for an RGB image, stored in row-major-order.
    /// The order matches that of numpy etc, and is ordered so that
    /// the "tighest wound" dimension is last.
    ///
    /// An empty shape means this tensor is a scale, i.e. of length 1.
    /// An empty vector has shape `[0]`, an empty matrix shape `[0, 0]`, etc.
    ///
    /// Conceptually `[h,w]` == `[h,w,1]` == `[h,w,1,1,1]` etc in most circumstances.
    pub shape: Vec<TensorDimension>,

    /// The per-element data format.
    /// numpy calls this `dtype`.
    pub dtype: TensorDataType,

    /// The actual contents of the tensor.
    pub data: TensorDataStore,
}

impl Tensor {
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
                for (TensorDimension { size, name: _ }, index) in self.shape.iter().zip(index).rev()
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
                    TensorDataType::F32 => TensorElement::F32(bytemuck::pod_read_unaligned(data)),
                })
            }
            TensorDataStore::Jpeg(_) => None, // Too expensive to unpack here.
        }
    }
}
