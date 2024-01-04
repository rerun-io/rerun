use crate::{view_kind::SpatialSpaceViewKind, visualizers::UiLabel};

/// Common data struct for all spatial scene elements.
///
/// Each spatial scene element is expected to fill an instance of this struct with its data.
pub struct SpatialViewVisualizerData {
    pub ui_labels: Vec<UiLabel>,
    pub bounding_box: macaw::BoundingBox,
    pub preferred_view_kind: Option<SpatialSpaceViewKind>,
}

impl SpatialViewVisualizerData {
    pub fn new(preferred_view_kind: Option<SpatialSpaceViewKind>) -> Self {
        Self {
            ui_labels: Vec::new(),
            bounding_box: macaw::BoundingBox::nothing(),
            preferred_view_kind,
        }
    }

    pub fn extend_bounding_box(
        &mut self,
        other: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        self.bounding_box = self
            .bounding_box
            .union(other.transform_affine3(&world_from_obj));
    }

    pub fn extend_bounding_box_with_points(
        &mut self,
        points: impl Iterator<Item = glam::Vec3>,
        world_from_obj: glam::Affine3A,
    ) {
        re_tracing::profile_function!();
        self.extend_bounding_box(macaw::BoundingBox::from_points(points), world_from_obj);
    }

    pub fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
