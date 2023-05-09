use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

/// A String label component
///
/// ```
/// use re_log_types::component_types::Label;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(Label::data_type(), DataType::Utf8);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct Label(pub String);

impl Label {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Component for Label {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.label".into()
    }
}

impl From<String> for Label {
    #[inline]
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<Label> for String {
    #[inline]
    fn from(value: Label) -> Self {
        value.0
    }
}

impl AsRef<str> for Label {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Label {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for Label {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
