use crate::datatypes::Vec3D;

use super::LineStrip3D;

// ---

impl LineStrip3D {
    /// Construct a line strip from a sequence of points.
    #[allow(clippy::should_implement_trait)] // vanilla `FromIter` is too limiting in what it can express
    pub fn from_iter(points: impl IntoIterator<Item = impl Into<Vec3D>>) -> Self {
        Self(points.into_iter().map(Into::into).collect())
    }
}
