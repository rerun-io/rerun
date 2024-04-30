use super::UVector3D;

impl UVector3D {
    /// The zero vector, i.e. the additive identity.
    pub const ZERO: Self = Self(crate::datatypes::UVec3D::ZERO);

    /// `[1, 1, 1]`, i.e. the multiplicative identity.
    pub const ONE: Self = Self(crate::datatypes::UVec3D::ONE);
}

#[cfg(feature = "glam")]
impl From<UVector3D> for glam::UVec3 {
    #[inline]
    fn from(v: UVector3D) -> Self {
        Self::new(v.x(), v.y(), v.z())
    }
}

#[cfg(feature = "mint")]
impl From<UVector3D> for mint::Vector3<u32> {
    #[inline]
    fn from(v: UVector3D) -> Self {
        Self {
            x: v.x(),
            y: v.y(),
            z: v.z(),
        }
    }
}
