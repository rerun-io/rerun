use super::Ellipses2D;
use crate::components::{HalfSize2D, Position2D};

impl Ellipses2D {
    /// Creates new [`Ellipses2D`] for circles with the given radii, centered around the local origin.
    // Note: This is not a `Radius` component because the `Radius` component is for
    // the on-screen sizes of lines and points.
    #[inline]
    #[doc(alias = "circle")]
    pub fn from_radii(radii: impl IntoIterator<Item = f32>) -> Self {
        Self::new(radii.into_iter().map(HalfSize2D::splat))
    }

    /// Creates new [`Ellipses2D`] for circles with [`Self::centers`] and the given radii.
    // Note: This is not a `Radius` component because the `Radius` component is for
    // the on-screen sizes of lines and points.
    #[inline]
    #[doc(alias = "circle")]
    pub fn from_centers_and_radii(
        centers: impl IntoIterator<Item = impl Into<Position2D>>,
        radii: impl IntoIterator<Item = f32>,
    ) -> Self {
        Self::new(radii.into_iter().map(HalfSize2D::splat)).with_centers(centers)
    }

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
