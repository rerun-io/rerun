#[cfg(feature = "glam")]
impl From<super::TriangleIndices> for glam::UVec3 {
    #[inline]
    fn from(v: super::TriangleIndices) -> Self {
        Self::new(v.x(), v.y(), v.z())
    }
}

#[cfg(feature = "mint")]
impl From<super::TriangleIndices> for mint::Vector3<u32> {
    #[inline]
    fn from(v: super::TriangleIndices) -> Self {
        Self {
            x: v.x(),
            y: v.y(),
            z: v.z(),
        }
    }
}
