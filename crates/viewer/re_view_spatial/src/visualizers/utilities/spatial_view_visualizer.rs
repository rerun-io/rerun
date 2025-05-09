use re_log_types::EntityPathHash;

use super::UiLabel;
use crate::{PickableTexturedRect, view_kind::SpatialViewKind, visualizers::LoadingSpinner};

/// Common data struct for all spatial scene elements.
///
/// Each spatial scene element is expected to fill an instance of this struct with its data.
pub struct SpatialViewVisualizerData {
    /// Loading icons/spinners shown using egui, in world/scene coordinates.
    pub loading_spinners: Vec<LoadingSpinner>,

    /// Labels that should be shown using egui.
    pub ui_labels: Vec<UiLabel>,

    /// Bounding boxes of all visualizations that the visualizer showed.
    pub bounding_boxes: Vec<(EntityPathHash, re_math::BoundingBox)>,

    /// Textured rectangles that the visualizer produced which can be interacted with.
    pub pickable_rects: Vec<PickableTexturedRect>,

    /// The view kind preferred by this visualizer (used for heuristics).
    pub preferred_view_kind: Option<SpatialViewKind>,
}

impl SpatialViewVisualizerData {
    pub fn new(preferred_view_kind: Option<SpatialViewKind>) -> Self {
        Self {
            loading_spinners: Default::default(),
            ui_labels: Default::default(),
            bounding_boxes: Default::default(),
            pickable_rects: Default::default(),
            preferred_view_kind,
        }
    }

    pub fn add_bounding_box(
        &mut self,
        entity: EntityPathHash,
        bbox: re_math::BoundingBox,
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
            re_math::BoundingBox::from_points(points),
            world_from_obj,
        );
    }

    pub fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
