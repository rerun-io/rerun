use crate::datatypes::Vec2D;

use super::Origin2D;

impl Origin2D {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self(Vec2D::new(x, y))
    }
}

#[cfg(feature = "glam")]
impl From<Origin2D> for glam::Vec2 {
    #[inline]
    fn from(pt: Origin2D) -> Self {
        Self::new(pt.x(), pt.y())
    }
}
