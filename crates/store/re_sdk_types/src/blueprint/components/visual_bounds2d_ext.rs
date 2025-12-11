use super::VisualBounds2D;
use crate::datatypes::Range2D;

impl From<VisualBounds2D> for emath::Rect {
    #[inline]
    fn from(v: VisualBounds2D) -> Self {
        Self::from(v.0)
    }
}

impl Default for VisualBounds2D {
    #[inline]
    fn default() -> Self {
        // Default that typically causes at least some content to be visible.
        Self(Range2D {
            x_range: [0.0, 100.0].into(),
            y_range: [0.0, 100.0].into(),
        })
    }
}
