use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

// TODO: explain why we keep that one (needed for annotation context)

/// A String label component
///
/// ```
/// use re_components::Label;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Label::data_type(), DataType::Utf8);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct LegacyLabel(pub String);

impl From<LegacyLabel> for re_types::components::Label {
    fn from(val: LegacyLabel) -> Self {
        re_types::components::Label(val.0)
    }
}

impl From<re_types::components::Label> for LegacyLabel {
    fn from(value: re_types::components::Label) -> Self {
        Self(value.0)
    }
}

impl LegacyLabel {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl re_log_types::LegacyComponent for LegacyLabel {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.label".into()
    }
}

impl From<&str> for LegacyLabel {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for LegacyLabel {
    #[inline]
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<LegacyLabel> for String {
    #[inline]
    fn from(value: LegacyLabel) -> Self {
        value.0
    }
}

impl AsRef<str> for LegacyLabel {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for LegacyLabel {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for LegacyLabel {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

re_log_types::component_legacy_shim!(LegacyLabel);
