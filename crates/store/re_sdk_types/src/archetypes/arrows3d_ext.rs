use super::Arrows3D;
use crate::components::Vector3D;

impl Arrows3D {
    /// Creates new 3D arrows pointing in the given directions, with a base at the origin (0, 0, 0).
    #[inline]
    pub fn from_vectors(vectors: impl IntoIterator<Item = impl Into<Vector3D>>) -> Self {
        Self::new(vectors)
    }
}
