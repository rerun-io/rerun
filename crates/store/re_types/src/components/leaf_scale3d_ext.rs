use crate::datatypes::Vec3D;

use super::LeafScale3D;

impl LeafScale3D {
    /// Scale the same amount along all axis.
    #[inline]
    pub fn uniform(value: f32) -> Self {
        Self(Vec3D([value, value, value]))
    }
}

impl From<f32> for LeafScale3D {
    #[inline]
    fn from(value: f32) -> Self {
        Self(crate::datatypes::Vec3D([value, value, value]))
    }
}

#[cfg(feature = "glam")]
impl From<LeafScale3D> for glam::Affine3A {
    #[inline]
    fn from(v: LeafScale3D) -> Self {
        Self {
            matrix3: glam::Mat3A::from_diagonal(v.0.into()),
            translation: glam::Vec3A::ZERO,
        }
    }
}

impl Default for LeafScale3D {
    #[inline]
    fn default() -> Self {
        Self(crate::datatypes::Vec3D([1.0, 1.0, 1.0]))
    }
}
