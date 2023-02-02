use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

// TODO(cmc): Points should just be containers of Vecs.

/// A point in 2D space.
///
/// ```
/// use re_log_types::component_types::Point2D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Point2D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const ZERO: Point2D = Point2D { x: 0.0, y: 0.0 };

    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Component for Point2D {
    fn name() -> crate::ComponentName {
        "rerun.point2d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Point2D> for glam::Vec2 {
    fn from(pt: Point2D) -> Self {
        Self::new(pt.x, pt.y)
    }
}

/// A point in 3D space.
///
/// ```
/// use re_log_types::component_types::Point3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Point3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///         Field::new("z", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub const ZERO: Point3D = Point3D {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl Component for Point3D {
    fn name() -> crate::ComponentName {
        "rerun.point3d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Point3D> for glam::Vec3 {
    fn from(pt: Point3D) -> Self {
        Self::new(pt.x, pt.y, pt.z)
    }
}
