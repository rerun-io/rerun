use crate::components::{HalfSize3D, Position3D};

use super::Ellipsoids;

impl Ellipsoids {
    /// Creates a new [`Ellipsoids`] with [`Self::half_sizes`].
    #[inline]
    pub fn from_half_sizes(half_sizes: impl IntoIterator<Item = impl Into<HalfSize3D>>) -> Self {
        Self::new(half_sizes)
    }

    /// Creates a new [`Ellipsoids`] for spheres with the given radii.
    // Note: This is not a `Radius` component because the `Radius` component is for
    // the on-screen sizes of lines and points.
    #[inline]
    #[doc(alias = "sphere")]
    pub fn from_radii(radii: impl IntoIterator<Item = f32>) -> Self {
        Self::new(radii.into_iter().map(sphere_size))
    }

    /// Creates a new [`Ellipsoids`] with [`Self::centers`] and [`Self::half_sizes`].
    #[inline]
    pub fn from_centers_and_half_sizes(
        centers: impl IntoIterator<Item = impl Into<Position3D>>,
        half_sizes: impl IntoIterator<Item = impl Into<HalfSize3D>>,
    ) -> Self {
        Self::new(half_sizes).with_centers(centers)
    }

    /// Creates a new [`Ellipsoids`] for spheres with the given [`Self::centers`], and
    /// [`Self::half_sizes`] all equal to the given radii.
    // Note: This is not a `Radius` component because the `Radius` component is for
    // the on-screen sizes of lines and points.
    #[doc(alias = "sphere")]
    #[inline]
    pub fn from_centers_and_radii(
        centers: impl IntoIterator<Item = impl Into<Position3D>>,
        radii: impl IntoIterator<Item = f32>,
    ) -> Self {
        Self::new(radii.into_iter().map(sphere_size)).with_centers(centers)
    }
}

fn sphere_size(radius: f32) -> HalfSize3D {
    HalfSize3D::new(radius, radius, radius)
}
