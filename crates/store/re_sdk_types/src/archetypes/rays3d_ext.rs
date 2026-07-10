use super::Rays3D;

impl Rays3D {
    /// Creates a new [`Rays3D`] with the given vectors and origins at (0, 0, 0).
    ///
    /// Each vector gives both the direction and finite rendered length of the ray.
    #[inline]
    pub fn from_vectors(
        vectors: impl IntoIterator<Item = impl Into<crate::components::Vector3D>>,
    ) -> Self {
        Self::new(vectors)
    }
}
