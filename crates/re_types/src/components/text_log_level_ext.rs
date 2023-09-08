use super::TextLogLevel;

impl TextLogLevel {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<TextLogLevel> for String {
    #[inline]
    fn from(value: TextLogLevel) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for TextLogLevel {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for TextLogLevel {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

// TODO(emilk): required to use with `range_entity_with_primary`. remove once the migration is over
impl arrow2_convert::field::ArrowField for TextLogLevel {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        use crate::Loggable as _;
        Self::arrow_field().data_type
    }
}
