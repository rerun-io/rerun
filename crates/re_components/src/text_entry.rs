use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

/// A text entry component, comprised of a text body and its log level.
///
/// ```
/// use re_components::TextEntry;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     TextEntry::data_type(),
///     DataType::Struct(vec![
///         Field::new("body", DataType::Utf8, false),
///         Field::new("level", DataType::Utf8, true),
///     ])
/// );
/// ```
#[derive(Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq, Eq)]
pub struct TextEntry {
    pub body: String,

    // Recommended to be one of:
    // * `"CRITICAL"`
    // * `"ERROR"`
    // * `"WARN"`
    // * `"INFO"`
    // * `"DEBUG"`
    // * `"TRACE"`
    pub level: Option<String>,
}

impl TextEntry {
    #[inline]
    pub fn new(body: impl Into<String>, level: Option<String>) -> Self {
        Self {
            body: body.into(),
            level,
        }
    }

    #[inline]
    pub fn from_body(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            level: None,
        }
    }
}

impl re_log_types::LegacyComponent for TextEntry {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.text_entry".into()
    }
}

re_log_types::component_legacy_shim!(TextEntry);
