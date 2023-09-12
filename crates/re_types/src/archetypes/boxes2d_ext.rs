use crate::{
    components::{HalfSizes2D, Origin2D},
    datatypes::Vec2D,
};

use super::Boxes2D;

impl Boxes2D {
    /// Creates new [`Boxes2D`] with [`Self::half_sizes`] centered around the local origin.
    #[inline]
    pub fn from_half_sizes(half_sizes: impl IntoIterator<Item = impl Into<HalfSizes2D>>) -> Self {
        Self::new(half_sizes)
    }

    /// Creates new [`Boxes2D`] with [`Self::centers`] and [`Self::half_sizes`].
    #[inline]
    pub fn from_centers_and_half_sizes(
        centers: impl IntoIterator<Item = impl Into<Origin2D>>,
        half_sizes: impl IntoIterator<Item = impl Into<HalfSizes2D>>,
    ) -> Self {
        Self::new(half_sizes).with_centers(centers)
    }

    /// Creates new [`Boxes2D`] with [`Self::half_sizes`] created from (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the input data.
    #[inline]
    pub fn from_sizes(sizes: impl IntoIterator<Item = impl Into<Vec2D>>) -> Self {
        Self::new(sizes.into_iter().map(|wh| {
            let wh = wh.into();
            HalfSizes2D::new(wh.x() / 2.0, wh.y() / 2.0)
        }))
    }

    /// Creates new [`Boxes2D`] with [`Self::centers`] and [`Self::half_sizes`] created from centers and (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the input data.
    #[inline]
    pub fn from_centers_and_sizes(
        centers: impl IntoIterator<Item = impl Into<Origin2D>>,
        sizes: impl IntoIterator<Item = impl Into<Vec2D>>,
    ) -> Self {
        Self::from_sizes(sizes).with_centers(centers)
    }

    /// Creates new [`Boxes2D`] with [`Self::centers`] and [`Self::half_sizes`] created from minimums and (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes from the input data.
    pub fn from_mins_and_sizes(
        mins: impl IntoIterator<Item = impl Into<Vec2D>>,
        sizes: impl IntoIterator<Item = impl Into<Vec2D>>,
    ) -> Self {
        let boxes = Self::from_sizes(sizes);
        let centers: Vec<_> = mins
            .into_iter()
            .zip(boxes.half_sizes.iter())
            .map(|(min, half_size)| {
                let min = min.into();
                Origin2D::new(min.x() + half_size.x(), min.y() + half_size.y())
            })
            .collect();
        boxes.with_centers(centers)
    }
}
