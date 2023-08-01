use super::Point2D;

// ---

impl Point2D {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[cfg(feature = "glam")]
impl From<Point2D> for glam::Vec2 {
    #[inline]
    fn from(pt: Point2D) -> Self {
        Self::new(pt.x, pt.y)
    }
}

#[cfg(feature = "glam")]
impl From<Point2D> for glam::Vec3 {
    #[inline]
    fn from(pt: Point2D) -> Self {
        Self::new(pt.x, pt.y, 0.0)
    }
}
