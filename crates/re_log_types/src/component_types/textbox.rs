use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

/// A text element intended to be displayed in a textbox
///
/// ```
/// use re_log_types::component_types::TextEntry;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     TextEntry::data_type(),
///     DataType::Struct(vec![
///         Field::new("body", DataType::Utf8, false),
///     ])
/// );
/// ```
// TODO(jleibs): Should this be reconciled with the `TextEntry` component?
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq, Eq)]
pub struct Textbox {
    // TODO(jleibs): Support options for advanced styling. HTML? Markdown?
    pub body: String,
}

impl Textbox {
    #[inline]
    pub fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }

    #[inline]
    pub fn from_body(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

impl Component for Textbox {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.textbox".into()
    }
}
