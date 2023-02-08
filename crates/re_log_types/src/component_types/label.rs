use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// A String label component
///
/// ```
/// use re_log_types::component_types::Label;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Label::data_type(), DataType::Utf8);
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    derive_more::From,
    derive_more::Into,
    ArrowField,
    ArrowSerialize,
    ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct Label(pub String);

impl Component for Label {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.label".into()
    }
}
