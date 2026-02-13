use super::{Range1D, Range2D};

impl Range2D {
    /// Range that expands from negative infinity to positive infinity on both axis.
    pub const INFINITY: Self = Self {
        x_range: Range1D::INFINITY,
        y_range: Range1D::INFINITY,
    };
}

impl From<emath::Rect> for Range2D {
    #[inline]
    fn from(rect: emath::Rect) -> Self {
        Self {
            x_range: rect.x_range().into(),
            y_range: rect.y_range().into(),
        }
    }
}

impl From<Range2D> for emath::Rect {
    #[inline]
    fn from(range2d: Range2D) -> Self {
        Self::from_x_y_ranges(range2d.x_range, range2d.y_range)
    }
}

impl std::fmt::Display for Range2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prec = f.precision().unwrap_or(crate::DEFAULT_DISPLAY_DECIMALS);
        write!(
            f,
            "[{:.prec$}, {:.prec$}]×[{:.prec$}, {:.prec$}]",
            self.x_range.0[0], self.x_range.0[1], self.y_range.0[0], self.y_range.0[1],
        )
    }
}
