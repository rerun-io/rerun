use arrow2_convert::ArrowField;

use crate::msg_bundle::Component;

/// A point in 2D space.
///
/// ```
/// use re_log_types::field_types::Point2D;
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
#[derive(Clone, Debug, ArrowField, PartialEq)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Component for Point2D {
    const NAME: crate::ComponentNameRef<'static> = "rerun.point2d";
}

/// A point in 3D space.
///
/// ```
/// use re_log_types::field_types::Point3D;
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
#[derive(Clone, Debug, ArrowField, PartialEq)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Component for Point3D {
    const NAME: crate::ComponentNameRef<'static> = "rerun.point3d";
}
