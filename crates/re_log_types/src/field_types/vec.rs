use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// A vector in 2D space.
///
/// ```
/// use re_log_types::field_types::Vec2D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Vec2D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct Vec2D {
    pub x: f32,
    pub y: f32,
}

impl Component for Vec2D {
    fn name() -> crate::ComponentName {
        "rerun.vec2d".into()
    }
}

/// A vector in 3D space.
///
/// ```
/// use re_log_types::field_types::Vec3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Vec3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///         Field::new("z", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Copy, Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Vec3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Component for Vec3D {
    fn name() -> crate::ComponentName {
        "rerun.vec3d".into()
    }
}

#[cfg(feature = "glam")]
impl From<Vec3D> for glam::Vec3 {
    fn from(v: Vec3D) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Vec3D {
    fn from(v: glam::Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}
