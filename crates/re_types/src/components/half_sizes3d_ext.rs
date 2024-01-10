use crate::datatypes::Vec3D;

use super::HalfSizes3D;

impl HalfSizes3D {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3D::new(x, y, z))
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
impl From<HalfSizes3D> for glam::Vec3 {
    #[inline]
    fn from(extent: HalfSizes3D) -> Self {
        Self::new(extent.x(), extent.y(), extent.z())
    }
}

#[cfg(feature = "mint")]
impl From<HalfSizes3D> for mint::Vector3<f32> {
    #[inline]
    fn from(extent: HalfSizes3D) -> Self {
        Self {
            x: extent.x(),
            y: extent.y(),
            z: extent.z(),
        }
    }
}
