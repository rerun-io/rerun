use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

// ---

/// A double-precision scalar.
///
/// ```
/// use re_log_types::field_types::Scalar;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Scalar::data_type(), DataType::Float64);
/// ```
#[derive(Debug, Clone, Copy, derive_more::From, derive_more::Into)]
pub struct Scalar(pub f64);

arrow_enable_vec_for_type!(Scalar);

impl ArrowField for Scalar {
    type Type = Self;
    fn data_type() -> arrow2::datatypes::DataType {
        <f64 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for Scalar {
    type MutableArrayType = <f64 as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        <f64 as ArrowSerialize>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        <f64 as ArrowSerialize>::arrow_serialize(&v.0, array)
    }
}

impl ArrowDeserialize for Scalar {
    type ArrayType = <f64 as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <f64 as ArrowDeserialize>::arrow_deserialize(v).map(Scalar)
    }
}

impl Component for Scalar {
    fn name() -> crate::ComponentName {
        "rerun.scalar".into()
    }
}

// ---

/// Additional properties of a scalar when rendered as a plot.
///
/// ```
/// use re_log_types::field_types::ScalarPlotProps;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     ScalarPlotProps::data_type(),
///     DataType::Struct(vec![
///         Field::new("scattered", DataType::Boolean, false),
///     ])
/// );
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    arrow2_convert::ArrowField,
    arrow2_convert::ArrowSerialize,
    arrow2_convert::ArrowDeserialize,
)]
pub struct ScalarPlotProps {
    pub scattered: bool,
}

impl Component for ScalarPlotProps {
    fn name() -> crate::ComponentName {
        "rerun.scalar_plot_props".into()
    }
}
