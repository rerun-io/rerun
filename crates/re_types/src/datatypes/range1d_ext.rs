use super::Range1D;

impl From<emath::Rangef> for Range1D {
    #[inline]
    fn from(rangef: emath::Rangef) -> Self {
        Self([rangef.min as f64, rangef.max as f64])
    }
}

impl From<Range1D> for emath::Rangef {
    #[inline]
    fn from(range1d: Range1D) -> Self {
        Self {
            min: range1d.0[0] as f32,
            max: range1d.0[1] as f32,
        }
    }
}
