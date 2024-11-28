use crate::datatypes::Utf8;

impl<T: Into<Utf8>> From<(T, T)> for super::Utf8Pair {
    #[inline(always)]
    fn from(value: (T, T)) -> Self {
        Self {
            first: value.0.into(),
            second: value.1.into(),
        }
    }
}

impl From<&(&str, &str)> for super::Utf8Pair {
    #[inline(always)]
    fn from(value: &(&str, &str)) -> Self {
        Self {
            first: value.0.into(),
            second: value.1.into(),
        }
    }
}
