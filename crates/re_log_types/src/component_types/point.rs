use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

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
