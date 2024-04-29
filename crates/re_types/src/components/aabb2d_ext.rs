use super::AABB2D;

impl From<AABB2D> for emath::Rect {
    #[inline]
    fn from(v: AABB2D) -> Self {
        Self {
            min: emath::pos2(v.min[0], v.min[1]),
            max: emath::pos2(v.max[0], v.max[1]),
        }
    }
}
