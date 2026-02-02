use super::Arrows2D;
use crate::components::Vector2D;

impl Arrows2D {
    /// Creates new 2D arrows pointing in the given directions, with a base at the origin (0, 0).
    #[inline]
    pub fn from_vectors(vectors: impl IntoIterator<Item = impl Into<Vector2D>>) -> Self {
        Self::new(vectors)
    }
}
