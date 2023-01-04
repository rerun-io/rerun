use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// A rectangle in 2D space.
///
/// ```
/// use re_log_types::field_types::Rect2D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Rect2D::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///         Field::new("w", DataType::Float32, false),
///         Field::new("h", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct Rect2D {
    /// Rect X-coordinate
    pub x: f32,
    /// Rect Y-coordinate
    pub y: f32,
    /// Box Width
    pub w: f32,
    /// Box Height
    pub h: f32,
}

impl Rect2D {
    #[inline]
    pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

impl Component for Rect2D {
    fn name() -> crate::ComponentName {
        "rerun.rect2d".into()
    }
}
