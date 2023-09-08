use super::Text;

impl Text {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<Text> for String {
    #[inline]
    fn from(value: Text) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for Text {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Text {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

// TODO(emilk): required to use with `range_entity_with_primary`. remove once the migration is over
impl arrow2_convert::field::ArrowField for Text {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        use crate::Loggable as _;
        Self::arrow_field().data_type
    }
}
