use arrow2_convert::ArrowField;

use crate::msg_bundle::Component;

/// A Quaternion represented by 4 real numbers.
///
/// ```
/// use re_log_types::field_types::Quaternion;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Quaternion::data_type(),
///     DataType::Struct(vec![
///         Field::new("x", DataType::Float32, false),
///         Field::new("y", DataType::Float32, false),
///         Field::new("z", DataType::Float32, false),
///         Field::new("w", DataType::Float32, false),
///     ])
/// );
/// ```
#[derive(Debug, ArrowField)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Component for Quaternion {
    fn name() -> crate::ComponentName {
        "rerun.quaternion".into()
    }
}
