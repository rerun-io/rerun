use super::HalfSize3D;
use crate::datatypes::Vec3D;

impl HalfSize3D {
    /// Create a new half-extent from half-width, half-height, half-depth.
    #[inline]
    pub const fn new(half_width: f32, half_height: f32, half_depth: f32) -> Self {
        Self(Vec3D::new(half_width, half_height, half_depth))
    }

    /// Create a new half-extent with all the same sizes (a radius, perhaps).
    #[inline]
    pub const fn splat(half_size: f32) -> Self {
        Self(Vec3D::new(half_size, half_size, half_size))
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

    /// Height of a box using this half-extent.
    #[inline]
    pub fn depth(self) -> f32 {
        self.z() * 2.0
    }

    /// Returns the min position of a box with these half-extents and a given center.
    ///
    /// In "image space axis semantics" (y-axis points down, x-axis points right), this is the top-left corner.
    #[cfg(feature = "glam")]
    pub fn box_min(self, box_center: super::Position3D) -> glam::Vec3 {
        glam::Vec3::from(box_center) - glam::Vec3::from(self)
    }

    /// Returns the maximum of a box with these half-extents and a given center.
    ///
    /// In "image space axis semantics" (y-axis points down, x-axis points right), this is the bottom-right corner.
    #[cfg(feature = "glam")]
    pub fn box_max(self, box_center: super::Position3D) -> glam::Vec3 {
        glam::Vec3::from(box_center) + glam::Vec3::from(self)
    }
}

#[cfg(feature = "glam")]
impl From<HalfSize3D> for glam::Vec3 {
    #[inline]
    fn from(extent: HalfSize3D) -> Self {
        Self::new(extent.x(), extent.y(), extent.z())
    }
}

#[cfg(feature = "mint")]
impl From<HalfSize3D> for mint::Vector3<f32> {
    #[inline]
    fn from(extent: HalfSize3D) -> Self {
        Self {
            x: extent.x(),
            y: extent.y(),
            z: extent.z(),
        }
    }
}

impl Default for HalfSize3D {
    #[inline]
    fn default() -> Self {
        Self(Vec3D::ONE)
    }
}
