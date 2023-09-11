use crate::{
    components::{HalfExtents2D, Origin2D},
    datatypes::Vec2D,
};

use super::Boxes2D;

impl Boxes2D {
    /// Creates new [`Boxes2D`] with [`Self::half_extents`] and [`Self::centers`] created from minimum X/Y and width + height.
    ///
    /// Does *not* preserve data as-is and instead creates centers and half-extents from the input data.
    pub fn from_xywh(
        xy: impl IntoIterator<Item = impl Into<Vec2D>>,
        extents: impl IntoIterator<Item = impl Into<Vec2D>>,
    ) -> Self {
        let half_extents: Vec<_> = extents
            .into_iter()
            .map(|wh| {
                let wh = wh.into();
                HalfExtents2D::new(wh.x() / 2.0, wh.y() / 2.0)
            })
            .collect();
        let centers: Vec<_> = xy
            .into_iter()
            .zip(half_extents.iter())
            .map(|(xy, half_extent)| {
                let xy = xy.into();
                Origin2D::new(xy.x() + half_extent.x(), xy.y() + half_extent.y())
            })
            .collect();

        Self::new(half_extents).with_centers(centers)
    }
}
