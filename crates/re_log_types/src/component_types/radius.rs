use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// A Radius component
///
/// ## Examples
///
/// ```
/// # use re_log_types::component_types::Radius;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(Radius::data_type(), DataType::Float32);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    derive_more::From,
    derive_more::Into,
    ArrowField,
    ArrowSerialize,
    ArrowDeserialize,
)]
#[arrow_field(transparent)]
pub struct Radius(pub f32);

impl Component for Radius {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.radius".into()
    }
}
