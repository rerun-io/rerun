use crate::datatypes::{self};

use super::Rotation3D;

impl Rotation3D {
    pub const IDENTITY: Self = Self(datatypes::Rotation3D::IDENTITY);
}

#[cfg(feature = "glam")]
impl From<Rotation3D> for glam::Quat {
    #[inline]
    fn from(val: Rotation3D) -> Self {
        val.0.into()
    }
}

#[cfg(feature = "mint")]
impl From<Rotation3D> for mint::Quaternion<f32> {
    #[inline]
    fn from(val: Rotation3D) -> Self {
        val.0.into()
    }
}
