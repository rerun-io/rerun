use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

/// A Radius component
///
/// ## Examples
///
/// ```
/// # use re_components::Radius;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(Radius::data_type(), DataType::Float32);
/// ```
#[derive(Debug, Clone, Copy, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct Radius(pub f32);

impl re_log_types::LegacyComponent for Radius {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.radius".into()
    }
}

re_log_types::component_legacy_shim!(Radius);
