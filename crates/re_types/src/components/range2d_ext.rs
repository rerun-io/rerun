use super::Range2D;

impl From<Range2D> for emath::Rect {
    #[inline]
    fn from(v: Range2D) -> Self {
        Self::from(v.0)
    }
}
