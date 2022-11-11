/// The built-in object types supported by Rerun.
///
/// In the future we will extend this to support user-defined types as well.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjectType {
    // A label and color associated with a particular class id
    ClassDescription,

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

    /// A 3D arrow
    Arrow3D,
}

impl ObjectType {
    pub fn members(self) -> &'static [&'static str] {
        #[allow(clippy::match_same_arms)]
        match self {
            Self::ClassDescription => &["id", "label", "color"],

            Self::TextEntry => &["color", "body", "level"],

            Self::Image => &["color", "tensor", "meter", "legend"],
            Self::Point2D => &["color", "pos", "radius"],
            Self::BBox2D => &["color", "bbox", "stroke_width", "label"],
            Self::LineSegments2D => &["color", "points", "stroke_width"],

            Self::Point3D => &["color", "pos", "radius"],
            Self::Box3D => &["color", "obb", "stroke_width", "label"],
            Self::Path3D => &["color", "points", "stroke_width"],
            Self::LineSegments3D => &["color", "points", "stroke_width"],
            Self::Mesh3D => &["color", "mesh"],
            Self::Arrow3D => &["color", "origin", "arrow3d", "width_scale", "label"],
        }
    }
}

/// These are fields not part of the actual object, but express meta-info about paths.
pub const META_FIELDS: &[&str] = &[
    "_annotations",
    "_transform",
    "_view_coordinates",
    "_visible",
];
