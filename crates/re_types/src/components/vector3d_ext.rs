use super::Vector3D;

impl Vector3D {
    pub const ZERO: Self = Self(crate::datatypes::Vec3D::ZERO);
    pub const ONE: Self = Self(crate::datatypes::Vec3D::ONE);
}

#[cfg(feature = "glam")]
impl From<Vector3D> for glam::Vec3 {
    #[inline]
    fn from(v: Vector3D) -> Self {
        Self::new(v.x(), v.y(), v.z())
    }
}

#[cfg(feature = "mint")]
impl From<Vector3D> for mint::Vector3<f32> {
    #[inline]
    fn from(v: Vector3D) -> Self {
        Self {
            x: v.x(),
            y: v.y(),
            z: v.z(),
        }
    }
}
