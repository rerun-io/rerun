use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

// ---

/// A double-precision scalar.
///
/// ## Examples
///
/// ```
/// # use re_log_types::component_types::Scalar;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(Scalar::data_type(), DataType::Float64);
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
pub struct Scalar(pub f64);

impl Component for Scalar {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.scalar".into()
    }
}

// ---

/// Additional properties of a scalar when rendered as a plot.
///
/// ## Examples
///
/// ```
/// # use re_log_types::component_types::ScalarPlotProps;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     ScalarPlotProps::data_type(),
///     DataType::Struct(vec![
///         Field::new("scattered", DataType::Boolean, false),
///     ])
/// );
/// ```
#[derive(Debug, Clone, Copy, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ScalarPlotProps {
    pub scattered: bool,
}

impl Component for ScalarPlotProps {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.scalar_plot_props".into()
    }
}
