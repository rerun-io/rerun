use super::GeoLineString;
use crate::datatypes::DVec2D;

// ---

impl GeoLineString {
    /// Create a new line string from a list of positions.
    #[expect(clippy::should_implement_trait)] // vanilla `FromIter` is too limiting in what it can express
    pub fn from_iter(points: impl IntoIterator<Item = impl Into<DVec2D>>) -> Self {
        Self(points.into_iter().map(Into::into).collect())
    }
}
