use crate::datatypes::Vec3D;

use super::Scale3D;

impl Scale3D {
    /// Scale the same amount along all axis.
    #[inline]
    pub fn uniform(value: f32) -> Self {
        Self(Vec3D([value, value, value]))
    }

    /// Scale the same amount along all axis.
    ///
    /// Deprecated method to mimic previous enum variant.
    #[allow(non_snake_case)]
    #[deprecated(since = "0.18.0", note = "Use `Scale3D::uniform` instead.")]
    pub fn Uniform(value: f32) -> Self {
        Self::uniform(value)
    }

    /// Scale individually along each axis.
    ///
    /// Deprecated method to mimic previous enum variant.
    #[allow(non_snake_case)]
    #[deprecated(since = "0.18.0", note = "Use `Scale3D::from` instead.")]
    pub fn ThreeD(value: impl Into<Vec3D>) -> Self {
        Self::from(value.into())
    }
}

impl From<f32> for Scale3D {
    #[inline]
    fn from(value: f32) -> Self {
        Self(crate::datatypes::Vec3D([value, value, value]))
    }
}

#[cfg(feature = "glam")]
impl From<Scale3D> for glam::Affine3A {
    #[inline]
    fn from(v: Scale3D) -> Self {
        Self {
            matrix3: glam::Mat3A::from_diagonal(v.0.into()),
            translation: glam::Vec3A::ZERO,
        }
    }
}

impl Default for Scale3D {
    #[inline]
    fn default() -> Self {
        Self(crate::datatypes::Vec3D([1.0, 1.0, 1.0]))
    }
}
