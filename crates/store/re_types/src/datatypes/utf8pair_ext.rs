use crate::datatypes::Utf8;

impl<T: Into<Utf8>> From<(T, T)> for super::Utf8Pair {
    fn from(value: (T, T)) -> Self {
        Self {
            first: value.0.into(),
            second: value.1.into(),
        }
    }
}
