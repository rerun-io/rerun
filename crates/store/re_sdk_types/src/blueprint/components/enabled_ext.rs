use super::Enabled;

impl From<Enabled> for bool {
    #[inline]
    fn from(v: Enabled) -> Self {
        v.0.into()
    }
}
