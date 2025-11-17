use super::Translation3D;
use crate::datatypes::Vec3D;

// This is intentionally not implemented for `DVec3`:
// The transform semantic is expressed here, `Vec3` on the other hand implements conversion to `glam::Vec3A`.
#[cfg(feature = "glam")]
impl From<Translation3D> for glam::Affine3A {
    #[inline]
    fn from(v: Translation3D) -> Self {
        Self {
            matrix3: glam::Mat3A::IDENTITY,
            translation: glam::Vec3A::from_slice(&v.0.0),
        }
    }
}

impl Translation3D {
    /// No translation.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// Create a new translation.
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3D::new(x, y, z))
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

    /// The z coordinate, i.e. index 2
    #[inline]
    pub fn z(&self) -> f32 {
        self.0.z()
    }
}
