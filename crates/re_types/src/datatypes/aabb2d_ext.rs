use super::AABB2D;

impl From<emath::Rect> for AABB2D {
    #[inline]
    fn from(v: emath::Rect) -> Self {
        Self {
            min: [v.min.x, v.min.y].into(),
            max: [v.max.x, v.max.y].into(),
        }
    }
}

impl From<AABB2D> for emath::Rect {
    #[inline]
    fn from(v: AABB2D) -> Self {
        Self {
            min: emath::pos2(v.min[0] as f32, v.min[1] as f32),
            max: emath::pos2(v.max[0] as f32, v.max[1] as f32),
        }
    }
}
