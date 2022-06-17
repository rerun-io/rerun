#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjectType {
    /// Information about a space (up axis etc).
    Space,

    Image,
    Point2D,
    BBox2D,
    LineSegments2D,

    Point3D,
    Box3D,
    Path3D,
    LineSegments3D,
    Mesh3D,
    Camera,
}

impl ObjectType {
    pub fn members(self) -> &'static [&'static str] {
        #[allow(clippy::match_same_arms)]
        match self {
            Self::Space => &["up"],

            Self::Image => &["space", "color", "image"],
            Self::Point2D => &["space", "color", "pos", "radius"],
            Self::BBox2D => &["space", "color", "bbox", "stroke_width"],
            Self::LineSegments2D => &["space", "color", "line_segments", "stroke_width"],

            Self::Point3D => &["space", "color", "pos", "radius"],
            Self::Box3D => &["space", "color", "obb", "stroke_width"],
            Self::Path3D => &["space", "color", "points", "stroke_width"],
            Self::LineSegments3D => &["space", "color", "line_segments", "stroke_width"],
            Self::Mesh3D => &["space", "color", "mesh"],
            Self::Camera => &["space", "color", "camera"],
        }
    }
}
