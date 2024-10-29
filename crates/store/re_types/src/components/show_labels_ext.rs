use super::ShowLabels;

impl From<ShowLabels> for bool {
    #[inline]
    fn from(value: ShowLabels) -> Self {
        value.0.into()
    }
}
