use super::Boxes2D;
use crate::components::{HalfSize2D, Position2D};
use crate::datatypes::Vec2D;

impl Boxes2D {
    /// Creates new [`Boxes2D`] with [`Self::half_sizes`] centered around the local origin.
    #[inline]
    pub fn from_half_sizes(half_sizes: impl IntoIterator<Item = impl Into<HalfSize2D>>) -> Self {
        Self::new(half_sizes)
    }

    /// Creates new [`Boxes2D`] with [`Self::centers`] and [`Self::half_sizes`].
    #[inline]
    pub fn from_centers_and_half_sizes(
        centers: impl IntoIterator<Item = impl Into<Position2D>>,
        half_sizes: impl IntoIterator<Item = impl Into<HalfSize2D>>,
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
            HalfSize2D::new(wh.x() / 2.0, wh.y() / 2.0)
        }))
    }

    /// Creates new [`Boxes2D`] with [`Self::centers`] and [`Self::half_sizes`] created from centers and (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the input data.
    #[inline]
    pub fn from_centers_and_sizes(
        centers: impl IntoIterator<Item = impl Into<Position2D>>,
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
        let half_sizes: Vec<_> = sizes
            .into_iter()
            .map(|size| {
                let size = size.into();
                HalfSize2D::new(size.x() / 2.0, size.y() / 2.0)
            })
            .collect();

        // The box semantics are such that the last half-size is used for all remaining boxes.
        if let Some(last_half_size) = half_sizes.last() {
            let centers: Vec<_> = mins
                .into_iter()
                .zip(half_sizes.iter().chain(std::iter::repeat(last_half_size)))
                .map(|(min, half_size)| {
                    let min = min.into();
                    Position2D::new(min.x() + half_size.x(), min.y() + half_size.y())
                })
                .collect();
            Self::from_half_sizes(half_sizes).with_centers(centers)
        } else {
            if mins.into_iter().next().is_some() {
                re_log::warn_once!("Must provide at least one size to create boxes.");
            }
            Self::from_half_sizes(half_sizes)
                .with_centers(std::iter::empty::<crate::components::Position2D>())
        }
    }
}
