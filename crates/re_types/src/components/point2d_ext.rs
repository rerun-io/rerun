use super::Point2D;

// ---

impl Point2D {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            xy: crate::datatypes::Point2D::new(x, y),
        }
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.xy.x
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.xy.y
    }
}

impl From<(f32, f32)> for Point2D {
    #[inline]
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

impl From<[f32; 2]> for Point2D {
    #[inline]
    fn from([x, y]: [f32; 2]) -> Self {
        Self::new(x, y)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for Point2D {
    #[inline]
    fn from(pt: glam::Vec2) -> Self {
        Self::new(pt.x, pt.y)
    }
}

#[cfg(feature = "glam")]
impl From<Point2D> for glam::Vec2 {
    #[inline]
    fn from(pt: Point2D) -> Self {
        Self::new(pt.x(), pt.y())
    }
}

#[cfg(feature = "glam")]
impl From<Point2D> for glam::Vec3 {
    #[inline]
    fn from(pt: Point2D) -> Self {
        Self::new(pt.x(), pt.y(), 0.0)
    }
}
