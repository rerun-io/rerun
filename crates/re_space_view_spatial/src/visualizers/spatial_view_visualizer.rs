use re_log_types::EntityPathHash;

use crate::{view_kind::SpatialSpaceViewKind, visualizers::UiLabel};

/// Common data struct for all spatial scene elements.
///
/// Each spatial scene element is expected to fill an instance of this struct with its data.
pub struct SpatialViewVisualizerData {
    pub ui_labels: Vec<UiLabel>,
    pub bounding_boxes: Vec<(EntityPathHash, macaw::BoundingBox)>,
    pub preferred_view_kind: Option<SpatialSpaceViewKind>,
}

impl SpatialViewVisualizerData {
    pub fn new(preferred_view_kind: Option<SpatialSpaceViewKind>) -> Self {
        Self {
            ui_labels: Vec::new(),
            bounding_boxes: Vec::new(),
            preferred_view_kind,
        }
    }

    pub fn add_bounding_box(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        self.bounding_boxes
            .push((entity, bbox.transform_affine3(&world_from_obj)));
    }

    pub fn add_bounding_box_from_points(
        &mut self,
        entity: EntityPathHash,
        points: impl Iterator<Item = glam::Vec3>,
        world_from_obj: glam::Affine3A,
    ) {
        re_tracing::profile_function!();
        self.add_bounding_box(
            entity,
            macaw::BoundingBox::from_points(points),
            world_from_obj,
        );
    }

    pub fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
