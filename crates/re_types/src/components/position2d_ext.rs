use crate::datatypes::Vec2D;

use super::Position2D;

// ---

impl Position2D {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self(Vec2D::new(x, y))
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.0.x()
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0.y()
    }
}

#[cfg(feature = "glam")]
impl From<Position2D> for glam::Vec2 {
    #[inline]
    fn from(pt: Position2D) -> Self {
        Self::new(pt.x(), pt.y())
    }
}

#[cfg(feature = "glam")]
impl From<Position2D> for glam::Vec3 {
    #[inline]
    fn from(pt: Position2D) -> Self {
        Self::new(pt.x(), pt.y(), 0.0)
    }
}

// TODO(#4690): this should be codegen'd.
impl crate::SizeBytes for Position2D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self(v) = self;
        v.heap_size_bytes()
    }
}

#[cfg(feature = "mint")]
impl From<Position2D> for mint::Point2<f32> {
    #[inline]
    fn from(position: Position2D) -> Self {
        Self {
            x: position.x(),
            y: position.y(),
        }
    }
}
