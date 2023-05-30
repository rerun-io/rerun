use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

/// A text element intended to be displayed in a text-box
///
/// ```
/// use re_components::TextBox;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     TextBox::data_type(),
///     DataType::Struct(vec![
///         Field::new("body", DataType::Utf8, false),
///     ])
/// );
/// ```
// TODO(jleibs): Should this be reconciled with the `TextEntry` component?
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq, Eq)]
pub struct TextBox {
    // TODO(jleibs): Support options for advanced styling. HTML? Markdown?
    pub body: String, // TODO(#1887): avoid allocations
}

impl TextBox {
    #[inline]
    pub fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }

    #[inline]
    pub fn from_body(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

impl re_log_types::Component for TextBox {
    #[inline]
    fn name() -> re_log_types::ComponentName {
        "rerun.text_box".into()
    }
}
