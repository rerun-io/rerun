use re_log_types::EntityPathHash;
use re_sdk_types::ViewClassIdentifier;
use re_viewer_context::ViewClass as _;

use super::UiLabel;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::LoadingSpinner;
use crate::{PickableTexturedRect, SpatialView2D};

/// Common data struct for all spatial scene elements.
///
/// Each spatial scene element is expected to fill an instance of this struct with its data.
pub struct SpatialViewVisualizerData {
    /// Loading icons/spinners shown using egui, in world/scene coordinates.
    pub loading_spinners: Vec<LoadingSpinner>,

    /// Labels that should be shown using egui.
    pub ui_labels: Vec<UiLabel>,

    /// Bounding boxes of all visualizations that the visualizer showed.
    pub bounding_boxes: Vec<(EntityPathHash, macaw::BoundingBox)>,

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

    pub fn add_pickable_rect(
        &mut self,
        pickable_rect: PickableTexturedRect,
        class_identifier: ViewClassIdentifier,
    ) {
        self.add_pickable_rect_to_bounding_box(&pickable_rect, class_identifier);
        self.pickable_rects.push(pickable_rect);
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

    pub fn add_pickable_rect_to_bounding_box(
        &mut self,
        pickable_rect: &PickableTexturedRect,
        class_identifier: ViewClassIdentifier,
    ) {
        // Only update the bounding box if this is a 2D view.
        // This is avoids a cyclic relationship where the image plane grows
        // the bounds which in turn influence the size of the image plane.
        // See: https://github.com/rerun-io/rerun/issues/3728
        if class_identifier == SpatialView2D::identifier() {
            self.bounding_boxes.push((
                pickable_rect.ent_path.hash(),
                pickable_rect.textured_rect.bounding_box(),
            ));
        }
    }

    pub fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
