use super::Scale3D;

impl From<crate::datatypes::Vec3D> for Scale3D {
    #[inline]
    fn from(v: crate::datatypes::Vec3D) -> Self {
        Self::ThreeD(v)
    }
}

impl From<f32> for Scale3D {
    #[inline]
    fn from(v: f32) -> Self {
        Self::Uniform(v)
    }
}

impl From<[f32; 3]> for Scale3D {
    #[inline]
    fn from(v: [f32; 3]) -> Self {
        Self::ThreeD(v.into())
    }
}

#[cfg(feature = "glam")]
impl From<Scale3D> for glam::Vec3 {
    #[inline]
    fn from(val: Scale3D) -> Self {
        match val {
            Scale3D::ThreeD(v) => v.into(),
            Scale3D::Uniform(v) => Self::splat(v),
        }
    }
}
