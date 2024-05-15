use super::VisualBounds2D;

impl From<VisualBounds2D> for emath::Rect {
    #[inline]
    fn from(v: VisualBounds2D) -> Self {
        Self::from(v.0)
    }
}
