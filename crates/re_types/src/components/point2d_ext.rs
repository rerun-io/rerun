use crate::datatypes::Vec2D;

use super::Point2D;

// ---

impl Point2D {
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

// NOTE: We have a ton of legacy tests that rely on the old APIs and `Point2D`.
// Since the new `Point2D` is binary compatible with the old we can easily drop the old one, but
// for that we need the new one to implement the `LegacyComponent` trait, which means implementing
// `ArrowField` to begin with!
// TODO(cmc): remove once the migration is over
impl arrow2_convert::field::ArrowField for Point2D {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        use crate::Loggable as _;
        Self::arrow_field().data_type
    }
}
