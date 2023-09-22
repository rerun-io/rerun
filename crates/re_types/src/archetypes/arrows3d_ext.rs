use crate::components::Vector3D;

use super::Arrows3D;

impl Arrows3D {
    /// Creates new 3D arrows pointing in the given directions.
    #[inline]
    pub fn from_vectors(vectors: impl IntoIterator<Item = impl Into<Vector3D>>) -> Self {
        Self::new(vectors)
    }
}
