use super::Planes3D;

impl Planes3D {
    /// Creates a new [`Planes3D`] with the given infinite planes.
    #[inline]
    pub fn from_planes(
        planes: impl IntoIterator<Item = impl Into<crate::components::Plane3D>>,
    ) -> Self {
        Self::new(planes)
    }
}
