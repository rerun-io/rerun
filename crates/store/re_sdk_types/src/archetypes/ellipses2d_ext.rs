use super::Ellipses2D;
use crate::components::{HalfSize2D, Position2D};

impl Ellipses2D {
    /// Creates new [`Ellipses2D`] with [`Self::half_sizes`] centered around the local origin.
    #[inline]
    pub fn from_half_sizes(half_sizes: impl IntoIterator<Item = impl Into<HalfSize2D>>) -> Self {
        Self::new(half_sizes)
    }

    /// Creates new [`Ellipses2D`] with [`Self::centers`] and [`Self::half_sizes`].
    #[inline]
    pub fn from_centers_and_half_sizes(
        centers: impl IntoIterator<Item = impl Into<Position2D>>,
        half_sizes: impl IntoIterator<Item = impl Into<HalfSize2D>>,
    ) -> Self {
        Self::new(half_sizes).with_centers(centers)
    }
}
