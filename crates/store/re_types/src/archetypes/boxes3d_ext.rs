use crate::{
    components::{HalfSize3D, PoseTranslation3D},
    datatypes::Vec3D,
};

use super::Boxes3D;

impl Boxes3D {
    /// Creates new [`Boxes3D`] with [`Self::half_sizes`] centered around the local origin.
    #[inline]
    pub fn from_half_sizes(half_sizes: impl IntoIterator<Item = impl Into<HalfSize3D>>) -> Self {
        Self::new(half_sizes)
    }

    /// Creates new [`Boxes3D`] with [`Self::centers`] and [`Self::half_sizes`].
    #[inline]
    pub fn from_centers_and_half_sizes(
        centers: impl IntoIterator<Item = impl Into<PoseTranslation3D>>,
        half_sizes: impl IntoIterator<Item = impl Into<HalfSize3D>>,
    ) -> Self {
        Self::new(half_sizes).with_centers(centers)
    }

    /// Creates new [`Boxes3D`] with [`Self::half_sizes`] created from (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the input data.
    #[inline]
    pub fn from_sizes(sizes: impl IntoIterator<Item = impl Into<Vec3D>>) -> Self {
        Self::new(sizes.into_iter().map(|size| {
            let wh = size.into();
            HalfSize3D::new(wh.x() / 2.0, wh.y() / 2.0, wh.z() / 2.0)
        }))
    }

    /// Creates new [`Boxes3D`] with [`Self::centers`] and [`Self::half_sizes`] created from centers and (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the input data.
    #[inline]
    pub fn from_centers_and_sizes(
        centers: impl IntoIterator<Item = impl Into<PoseTranslation3D>>,
        sizes: impl IntoIterator<Item = impl Into<Vec3D>>,
    ) -> Self {
        Self::from_sizes(sizes).with_centers(centers)
    }

    /// Creates new [`Boxes3D`] with [`Self::centers`] and [`Self::half_sizes`] created from minimums and (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes from the input data.
    pub fn from_mins_and_sizes(
        mins: impl IntoIterator<Item = impl Into<Vec3D>>,
        sizes: impl IntoIterator<Item = impl Into<Vec3D>>,
    ) -> Self {
        let boxes = Self::from_sizes(sizes);

        // The box semantics are such that the last half-size is used for all remaining boxes.
        if let Some(last_half_size) = boxes.half_sizes.last() {
            let centers: Vec<_> = mins
                .into_iter()
                .zip(
                    boxes
                        .half_sizes
                        .iter()
                        .chain(std::iter::repeat(last_half_size)),
                )
                .map(|(min, half_size)| {
                    let min = min.into();
                    PoseTranslation3D::new(
                        min.x() + half_size.x(),
                        min.y() + half_size.y(),
                        min.z() + half_size.z(),
                    )
                })
                .collect();
            boxes.with_centers(centers)
        } else {
            if mins.into_iter().next().is_some() {
                re_log::warn_once!("Must provide at least one size to create boxes.");
            }
            boxes.with_centers(std::iter::empty::<crate::components::PoseTranslation3D>())
        }
    }
}
