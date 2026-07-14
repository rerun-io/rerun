use re_log_types::EntityPathHash;
use re_sdk_types::ViewClassIdentifier;
use re_viewer_context::{SystemExecutionOutput, ViewClass as _};

use super::UiLabel;
use crate::visualizers::LoadingIndicator;
use crate::{PickableTexturedRect, SpatialView2D};

/// Common data struct for all spatial scene elements.
///
/// Each spatial scene element is expected to fill an instance of this struct with its data.
#[derive(Default)]
pub struct SpatialViewVisualizerData {
    /// Loading indicators shown using egui, in world/scene coordinates.
    pub loading_indicators: Vec<LoadingIndicator>,

    /// Labels that should be shown using egui.
    pub ui_labels: Vec<UiLabel>,

    /// Bounding boxes of all visualizations that the visualizer showed.
    bounding_boxes: Vec<(EntityPathHash, macaw::BoundingBox)>,

    /// Regions of interest for all visualizations, excluding spatial outliers.
    ///
    /// Used for camera framing and other heuristics. For most visualizers this is
    /// identical to the bounding box. Point cloud visualizers may provide a tighter
    /// region that excludes outlier points.
    regions_of_interest: Vec<(EntityPathHash, macaw::BoundingBox)>,

    /// Textured rectangles that the visualizer produced which can be interacted with.
    pub pickable_rects: Vec<PickableTexturedRect>,
}

impl SpatialViewVisualizerData {
    pub fn add_pickable_rect(
        &mut self,
        pickable_rect: PickableTexturedRect,
        class_identifier: ViewClassIdentifier,
    ) {
        self.add_pickable_rect_to_bounding_box(&pickable_rect, class_identifier);
        self.pickable_rects.push(pickable_rect);
    }

    /// Adds a bounding box and region of interest for an entity.
    ///
    /// For most visualizers these are the same. Use [`Self::add_bounding_box_and_region_of_interest`]
    /// when they differ (e.g. for point clouds with outlier rejection).
    pub fn add_bounding_box(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        let transformed = bbox.transform_affine3(&world_from_obj);
        self.bounding_boxes.push((entity, transformed));
        self.regions_of_interest.push((entity, transformed));
    }

    /// Adds separate bounding box and region of interest for an entity.
    ///
    /// The bounding box is the exact extent; the region of interest excludes outliers
    /// and is used for camera framing and other heuristics.
    pub fn add_bounding_box_and_region_of_interest(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        region_of_interest: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        self.bounding_boxes
            .push((entity, bbox.transform_affine3(&world_from_obj)));
        self.regions_of_interest.push((
            entity,
            region_of_interest.transform_affine3(&world_from_obj),
        ));
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
            let entry = (
                pickable_rect.ent_path.hash(),
                pickable_rect.textured_rect.bounding_box(),
            );
            self.bounding_boxes.push(entry);
            self.regions_of_interest.push(entry);
        }
    }

    pub fn iter_bounding_boxes(
        &self,
    ) -> impl ExactSizeIterator<Item = &(EntityPathHash, macaw::BoundingBox)> {
        self.bounding_boxes.iter()
    }

    pub fn iter_regions_of_interest(
        &self,
    ) -> impl ExactSizeIterator<Item = &(EntityPathHash, macaw::BoundingBox)> {
        self.regions_of_interest.iter()
    }
}

/// Iterate over [`SpatialViewVisualizerData`] from all visualizer outputs,
/// paired with the affinity of the visualizer that produced it.
pub fn iter_spatial_data(
    system_output: &SystemExecutionOutput,
) -> impl Iterator<Item = (Option<ViewClassIdentifier>, &SpatialViewVisualizerData)> {
    system_output
        .visualizer_execution_output
        .per_visualizer
        .values()
        .filter_map(|result| {
            let output = result.as_ref().ok()?;
            let data = output.get_visualizer_data::<SpatialViewVisualizerData>()?;
            Some((output.affinity, data))
        })
}
