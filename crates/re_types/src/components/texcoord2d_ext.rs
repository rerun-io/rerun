use crate::datatypes::Vec2D;

use super::Texcoord2D;

// ---

impl Texcoord2D {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0);

    #[inline]
    pub const fn new(u: f32, v: f32) -> Self {
        Self(Vec2D::new(u, v))
    }

    #[inline]
    pub fn u(&self) -> f32 {
        self.0.x()
    }

    #[inline]
    pub fn v(&self) -> f32 {
        self.0.y()
    }
}

#[cfg(feature = "glam")]
impl From<Texcoord2D> for glam::Vec2 {
    #[inline]
    fn from(pt: Texcoord2D) -> Self {
        Self::new(pt.u(), pt.v())
    }
}

#[cfg(feature = "mint")]
impl From<Texcoord2D> for mint::Point2<f32> {
    #[inline]
    fn from(position: Texcoord2D) -> Self {
        Self {
            x: position.u(),
            y: position.v(),
        }
    }
}

#[cfg(feature = "mint")]
impl From<mint::Point2<f32>> for Texcoord2D {
    #[inline]
    fn from(position: mint::Point2<f32>) -> Self {
        Self(Vec2D([position.x, position.y]))
    }
}
