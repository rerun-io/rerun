use super::Position2D;
use crate::datatypes::Vec2D;

// ---

impl Position2D {
    /// The origin.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// Create a new position.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self(Vec2D::new(x, y))
    }

    /// The x coordinate, i.e. index 0
    #[inline]
    pub fn x(&self) -> f32 {
        self.0.x()
    }

    /// The y coordinate, i.e. index 1
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

#[cfg(feature = "mint")]
impl From<mint::Point2<f32>> for Position2D {
    #[inline]
    fn from(position: mint::Point2<f32>) -> Self {
        Self(Vec2D([position.x, position.y]))
    }
}
