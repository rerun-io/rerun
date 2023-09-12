use crate::{
    components::{HalfSizes2D, Origin2D},
    datatypes::Vec2D,
};

use super::Boxes2D;

impl Boxes2D {
    /// Creates new [`Boxes2D`] with [`Self::half_sizes`] and [`Self::centers`] created from minimums and (full) sizes.
    ///
    /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes from the input data.
    pub fn from_mins_and_sizes(
        mins: impl IntoIterator<Item = impl Into<Vec2D>>,
        sizes: impl IntoIterator<Item = impl Into<Vec2D>>,
    ) -> Self {
        let half_sizes: Vec<_> = sizes
            .into_iter()
            .map(|wh| {
                let wh = wh.into();
                HalfSizes2D::new(wh.x() / 2.0, wh.y() / 2.0)
            })
            .collect();
        let centers: Vec<_> = mins
            .into_iter()
            .zip(half_sizes.iter())
            .map(|(min, half_size)| {
                let min = min.into();
                Origin2D::new(min.x() + half_size.x(), min.y() + half_size.y())
            })
            .collect();

        Self::new(half_sizes).with_centers(centers)
    }
}
