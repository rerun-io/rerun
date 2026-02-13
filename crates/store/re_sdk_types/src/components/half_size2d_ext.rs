use super::HalfSize2D;
use crate::datatypes::Vec2D;

impl HalfSize2D {
    /// Create a new half-extent from half-width and half-height.
    #[inline]
    pub const fn new(half_width: f32, half_height: f32) -> Self {
        Self(Vec2D::new(half_width, half_height))
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

    /// Returns the min position of a box with these half-extents and a given center.
    ///
    /// In "image space axis semantics" (y-axis points down, x-axis points right), this is the top-left corner.
    #[cfg(feature = "glam")]
    pub fn box_min(self, box_center: super::Position2D) -> glam::Vec2 {
        glam::Vec2::from(box_center) - glam::Vec2::from(self)
    }

    /// Returns the maximum of a box with these half-extents and a given center.
    ///
    /// In "image space axis semantics" (y-axis points down, x-axis points right), this is the bottom-right corner.
    #[cfg(feature = "glam")]
    pub fn box_max(self, box_center: super::Position2D) -> glam::Vec2 {
        glam::Vec2::from(box_center) + glam::Vec2::from(self)
    }
}

#[cfg(feature = "glam")]
impl From<HalfSize2D> for glam::Vec2 {
    #[inline]
    fn from(extent: HalfSize2D) -> Self {
        Self::new(extent.x(), extent.y())
    }
}

#[cfg(feature = "glam")]
impl From<HalfSize2D> for glam::Vec3 {
    #[inline]
    fn from(extent: HalfSize2D) -> Self {
        Self::new(extent.x(), extent.y(), 0.0)
    }
}

#[cfg(feature = "mint")]
impl From<HalfSize2D> for mint::Vector2<f32> {
    #[inline]
    fn from(extent: HalfSize2D) -> Self {
        Self {
            x: extent.x(),
            y: extent.y(),
        }
    }
}

impl Default for HalfSize2D {
    #[inline]
    fn default() -> Self {
        Self(Vec2D::ONE)
    }
}
