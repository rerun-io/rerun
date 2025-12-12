use super::Ellipsoids3D;
use crate::components::{HalfSize3D, Translation3D};

impl Ellipsoids3D {
    /// Creates a new [`Ellipsoids3D`] for spheres with the given radii.
    // Note: This is not a `Radius` component because the `Radius` component is for
    // the on-screen sizes of lines and points.
    #[inline]
    #[doc(alias = "sphere")]
    pub fn from_radii(radii: impl IntoIterator<Item = f32>) -> Self {
        Self::new(radii.into_iter().map(HalfSize3D::splat))
    }

    /// Creates a new [`Ellipsoids3D`] for spheres with the given [`Self::centers`], and
    /// [`Self::half_sizes`] all equal to the given radii.
    // Note: This is not a `Radius` component because the `Radius` component is for
    // the on-screen sizes of lines and points.
    #[doc(alias = "sphere")]
    #[inline]
    pub fn from_centers_and_radii(
        centers: impl IntoIterator<Item = impl Into<Translation3D>>,
        radii: impl IntoIterator<Item = f32>,
    ) -> Self {
        Self::new(radii.into_iter().map(HalfSize3D::splat)).with_centers(centers)
    }

    /// Creates a new [`Ellipsoids3D`] with [`Self::half_sizes`].
    #[inline]
    pub fn from_half_sizes(half_sizes: impl IntoIterator<Item = impl Into<HalfSize3D>>) -> Self {
        Self::new(half_sizes)
    }

    /// Creates a new [`Ellipsoids3D`] with [`Self::centers`] and [`Self::half_sizes`].
    #[inline]
    pub fn from_centers_and_half_sizes(
        centers: impl IntoIterator<Item = impl Into<Translation3D>>,
        half_sizes: impl IntoIterator<Item = impl Into<HalfSize3D>>,
    ) -> Self {
        Self::new(half_sizes).with_centers(centers)
    }
}
