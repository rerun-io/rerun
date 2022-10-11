/// The built-in object types supported by Rerun.
///
/// In the future we will extend this to support user-defined types as well.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjectType {
    /// Information about a space (up axis etc).
    Space,
    // A colored label that can be used for Segmentation Masks
    SegmentationLabel,

    /// A logging message.
    TextEntry,

    /// An image. Could be gray, RGB, a depth map, ….
    Image,

    /// A point in 2D space.
    Point2D,

    /// 2D rectangle.
    BBox2D,

    /// Many 2D line segments.
    LineSegments2D,

    /// A point in 3D space.
    Point3D,

    /// 3D oriented bounding box (OBB).
    Box3D,

    /// A path through 3D space.
    Path3D,

    /// Many 3D line segments.
    LineSegments3D,

    /// A 3D mesh.
    Mesh3D,

    /// Camera extrinsics and intrinsics.
    Camera,

    /// A 3D arrow
    Arrow3D,
}

impl ObjectType {
    pub fn members(self) -> &'static [&'static str] {
        #[allow(clippy::match_same_arms)]
        match self {
            Self::Space => &["up"],
            Self::SegmentationLabel => &["index", "label", "color"],

            Self::TextEntry => &["space", "color", "body", "level"],

            Self::Image => &["space", "color", "tensor", "meter"],
            Self::Point2D => &["space", "color", "pos", "radius"],
            Self::BBox2D => &["space", "color", "bbox", "stroke_width", "label"],
            Self::LineSegments2D => &["space", "color", "points", "stroke_width"],

            Self::Point3D => &["space", "color", "pos", "radius"],
            Self::Box3D => &["space", "color", "obb", "stroke_width", "label"],
            Self::Path3D => &["space", "color", "points", "stroke_width"],
            Self::LineSegments3D => &["space", "color", "points", "stroke_width"],
            Self::Mesh3D => &["space", "color", "mesh"],
            Self::Camera => &["space", "color", "camera"],
            Self::Arrow3D => &[
                "space",
                "color",
                "origin",
                "arrow3d",
                "width_scale",
                "label",
            ],
        }
    }
}

/// These are fields not part of the actual object, but express meta-info about paths.
pub const META_FIELDS: &[&str] = &["_transform", "_visible"];
