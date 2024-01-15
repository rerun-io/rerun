use crate::datatypes::Quaternion;

use super::Rotation3D;

impl Rotation3D {
    pub const IDENTITY: Rotation3D = Rotation3D::Quaternion(Quaternion::IDENTITY);
}

impl From<Quaternion> for Rotation3D {
    #[inline]
    fn from(q: Quaternion) -> Self {
        Self::Quaternion(q)
    }
}

impl From<crate::datatypes::RotationAxisAngle> for Rotation3D {
    #[inline]
    fn from(r: crate::datatypes::RotationAxisAngle) -> Self {
        Self::AxisAngle(r)
    }
}

#[cfg(feature = "glam")]
impl From<Rotation3D> for glam::Quat {
    #[inline]
    fn from(val: Rotation3D) -> Self {
        match val {
            Rotation3D::Quaternion(v) => v.into(),
            Rotation3D::AxisAngle(a) => a.into(),
        }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Quat> for Rotation3D {
    #[inline]
    fn from(val: glam::Quat) -> Self {
        Rotation3D::Quaternion(val.into())
    }
}

#[cfg(feature = "mint")]
impl From<Rotation3D> for mint::Quaternion<f32> {
    #[inline]
    fn from(val: Rotation3D) -> Self {
        match val {
            Rotation3D::Quaternion(v) => v.into(),
            Rotation3D::AxisAngle(a) => a.into(),
        }
    }
}

#[cfg(feature = "mint")]
impl From<mint::Quaternion<f32>> for Rotation3D {
    #[inline]
    fn from(val: mint::Quaternion<f32>) -> Self {
        Rotation3D::Quaternion(val.into())
    }
}
