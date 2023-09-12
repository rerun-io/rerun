use crate::datatypes::Vec2D;

use super::HalfExtents2D;

impl HalfExtents2D {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self(Vec2D::new(x, y))
    }

    #[inline]
    pub fn x(self) -> f32 {
        self.0.x()
    }

    #[inline]
    pub fn y(self) -> f32 {
        self.0.y()
    }

    /// Width of a box using this half-extent.
    #[inline]
    pub fn width(self) -> f32 {
        self.x() * 2.0
    }

    /// Height of a box using this half-extent.
    #[inline]
    pub fn height(self) -> f32 {
        self.y() * 2.0
    }

    /// The whole extent, i.e. twice the half extents.
    #[cfg(feature = "glam")]
    #[inline]
    pub fn full_extents(self) -> glam::Vec2 {
        glam::Vec2::from(self) * 2.0
    }

    /// Returns the min position of a box with these half-extents and a given center.
    ///
    /// In "image space axis semantics" (y-axis points down, x-axis points right), this is the top-left corner.
    #[cfg(feature = "glam")]
    pub fn box_min(self, box_center: super::Origin2D) -> glam::Vec2 {
        glam::Vec2::from(box_center) - glam::vec2(self.x(), self.y())
    }

    /// Returns the maximum of a box with these half-extents and a given center.
    ///
    /// In "image space axis semantics" (y-axis points down, x-axis points right), this is the bottom-right corner.
    #[cfg(feature = "glam")]
    pub fn box_max(self, box_center: super::Origin2D) -> glam::Vec2 {
        glam::Vec2::from(box_center) + glam::vec2(self.x(), self.y())
    }
}

#[cfg(feature = "glam")]
impl From<HalfExtents2D> for glam::Vec2 {
    #[inline]
    fn from(extent: HalfExtents2D) -> Self {
        Self::new(extent.x(), extent.y())
    }
}

#[cfg(feature = "glam")]
impl From<HalfExtents2D> for glam::Vec3 {
    #[inline]
    fn from(extent: HalfExtents2D) -> Self {
        Self::new(extent.x(), extent.y(), 0.0)
    }
}
