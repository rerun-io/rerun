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
    Box3(Box3),
    Path3D(Vec<[f32; 3]>),
    /// Special sibling attributes: "color", "radius"
    LineSegments3D(Vec<[[f32; 3]; 2]>),
    Mesh3D(Mesh3D),

    // ----------------------------
    // N-D:
    Vecf32(Vec<f32>),
}

impl Data {
    pub fn is_2d(&self) -> bool {
        match self {
            Self::I32(_)
            | Self::F32(_)
            | Self::Color(_)
            | Self::Pos3(_)
            | Self::Box3(_)
            | Self::Path3D(_)
            | Self::LineSegments3D(_)
            | Self::Mesh3D(_)
            | Self::Vecf32(_) => false,

            Self::Pos2(_) | Self::LineSegments2D(_) | Self::BBox2D(_) | Self::Image(_) => true,
        }
    }

    pub fn is_3d(&self) -> bool {
        match self {
            Self::I32(_)
            | Self::F32(_)
            | Self::Color(_)
            | Self::Pos2(_)
            | Self::LineSegments2D(_)
            | Self::BBox2D(_)
            | Self::Image(_)
            | Self::Vecf32(_) => false,

            Self::Pos3(_)
            | Self::Box3(_)
            | Self::Path3D(_)
            | Self::LineSegments3D(_)
            | Self::Mesh3D(_) => true,
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
pub enum MeshFormat {
    Gltf,
    Glb,
    Obj,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Mesh3D {
    pub format: MeshFormat,
    pub bytes: std::sync::Arc<[u8]>,
    /// four columns of a transformation matrix
    pub transform: [[f32; 4]; 4],
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ImageFormat {
    Luminance8,
    Rgba8,
    Jpeg,
}

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Image {
    // TODO: pub pos: [f32; 2],
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
