use super::TriangleIndices;

impl TriangleIndices {
    /// The zero vector, i.e. the additive identity.
    pub const ZERO: Self = Self(crate::datatypes::UVec3D::ZERO);

    /// `[1, 1, 1]`, i.e. the multiplicative identity.
    pub const ONE: Self = Self(crate::datatypes::UVec3D::ONE);
}

#[cfg(feature = "glam")]
impl From<TriangleIndices> for glam::UVec3 {
    #[inline]
    fn from(v: TriangleIndices) -> Self {
        Self::new(v.x(), v.y(), v.z())
    }
}

#[cfg(feature = "mint")]
impl From<TriangleIndices> for mint::Vector3<u32> {
    #[inline]
    fn from(v: TriangleIndices) -> Self {
        Self {
            x: v.x(),
            y: v.y(),
            z: v.z(),
        }
    }
}
