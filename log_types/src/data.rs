use crate::impl_into_enum;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Data {
    // 1D:
    I32(i32),
    F32(f32),

    /// RGBA unmultiplied/separate alpha
    Color([u8; 4]),

    // ----------------------------
    // 2D:
    /// Special sibling attributes: "color", "radius"
    Pos2([f32; 2]),
    /// Special sibling attributes: "color"
    BBox2D(BBox2D),
    LineSegments2D(Vec<[[f32; 2]; 2]>),
    Image(Image),

    // ----------------------------
    // 3D:
    /// Special sibling attributes: "color", "radius"
    Pos3([f32; 3]),
    /// Used for specifying the "up" axis of a 3D space
    Vec3([f32; 3]),
    Box3(Box3),
    Path3D(Vec<[f32; 3]>),
    /// Special sibling attributes: "color", "radius"
    LineSegments3D(Vec<[[f32; 3]; 2]>),
    Mesh3D(Mesh3D),
    Camera(Camera),

    // ----------------------------
    // N-D:
    Vecf32(Vec<f32>),
}

impl Data {
    #[inline]
    pub fn typ(&self) -> DataType {
        match self {
            Self::I32(_) => DataType::I32,
            Self::F32(_) => DataType::F32,
            Self::Color(_) => DataType::Color,

            Self::Pos2(_) => DataType::Pos2,
            Self::BBox2D(_) => DataType::BBox2D,
            Self::LineSegments2D(_) => DataType::LineSegments2D,
            Self::Image(_) => DataType::Image,

            Self::Pos3(_) => DataType::Pos3,
            Self::Vec3(_) => DataType::Vec3,
            Self::Box3(_) => DataType::Box3,
            Self::Path3D(_) => DataType::Path3D,
            Self::LineSegments3D(_) => DataType::LineSegments3D,
            Self::Mesh3D(_) => DataType::Mesh3D,
            Self::Camera(_) => DataType::Camera,

            Self::Vecf32(_) => DataType::Vecf32,
        }
    }

    #[inline]
    pub fn is_2d(&self) -> bool {
        self.typ().is_2d()
    }

    #[inline]
    pub fn is_3d(&self) -> bool {
        self.typ().is_3d()
    }

    /// The center of this 3D thing, if any
    pub fn center3d(&self) -> Option<[f32; 3]> {
        match self {
            Self::Pos3(pos) => Some(*pos),
            Self::Box3(bbox) => Some(bbox.translation),
            Self::Path3D(points) => {
                let mut sum = [0.0_f64; 3];
                for point in points {
                    sum[0] += point[0] as f64;
                    sum[1] += point[1] as f64;
                    sum[2] += point[2] as f64;
                }
                let n = points.len() as f32;
                if n == 0.0 {
                    None
                } else {
                    Some([sum[0] as f32 / n, sum[1] as f32 / n, sum[2] as f32 / n])
                }
            }
            Self::LineSegments3D(segments) => {
                let mut sum = [0.0_f64; 3];
                for segment in segments {
                    for point in segment {
                        sum[0] += point[0] as f64;
                        sum[1] += point[1] as f64;
                        sum[2] += point[2] as f64;
                    }
                }
                let n = 2.0 * segments.len() as f32;
                if n == 0.0 {
                    None
                } else {
                    Some([sum[0] as f32 / n, sum[1] as f32 / n, sum[2] as f32 / n])
                }
            }
            Self::Mesh3D(_) => {
                None // TODO
            }
            Self::Camera(cam) => Some(cam.position),
            _ => None,
        }
    }
}

impl_into_enum!(i32, Data, I32);
impl_into_enum!(f32, Data, F32);
impl_into_enum!(BBox2D, Data, BBox2D);
impl_into_enum!(Vec<f32>, Data, Vecf32);
impl_into_enum!(Image, Data, Image);
impl_into_enum!(Mesh3D, Data, Mesh3D);

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataType {
    // 1D:
    I32,
    F32,

    Color,

    // ----------------------------
    // 2D:
    Pos2,
    BBox2D,
    LineSegments2D,
    Image,

    // ----------------------------
    // 3D:
    Pos3,
    Vec3,
    Box3,
    Path3D,
    LineSegments3D,
    Mesh3D,
    Camera,

    // ----------------------------
    // N-D:
    Vecf32,
}

impl DataType {
    #[inline]
    pub fn dimensionality(&self) -> Option<u32> {
        match self {
            Self::I32 | Self::F32 => Some(1),

            Self::Pos2 | Self::BBox2D | Self::LineSegments2D | Self::Image => Some(2),

            Self::Pos3
            | Self::Vec3
            | Self::Box3
            | Self::Path3D
            | Self::LineSegments3D
            | Self::Mesh3D
            | Self::Camera => Some(3),

            Self::Color | Self::Vecf32 => None,
        }
    }

    #[inline]
    pub fn is_2d(&self) -> bool {
        self.dimensionality() == Some(2)
    }

    #[inline]
    pub fn is_3d(&self) -> bool {
        self.dimensionality() == Some(3)
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

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ImageFormat {
    Luminance8,
    Rgb8,
    Rgba8,
    Jpeg,
}

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Image {
    // TODO: pub pos: [f32; 2], or a transform matrix
    /// Must always be set and correct, even for [`ImageFormat::Jpeg`].
    pub size: [u32; 2],
    pub format: ImageFormat,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("size", &self.size)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}
