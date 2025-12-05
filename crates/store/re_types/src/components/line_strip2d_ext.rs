use super::LineStrip2D;
use crate::datatypes::Vec2D;

// ---

impl LineStrip2D {
    /// Create a new line strip from a list of positions.
    #[expect(clippy::should_implement_trait)] // vanilla `FromIter` is too limiting in what it can express
    pub fn from_iter(points: impl IntoIterator<Item = impl Into<Vec2D>>) -> Self {
        Self(points.into_iter().map(Into::into).collect())
    }
}
