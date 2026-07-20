use re_log_types::EntityPathHash;
use re_viewer_context::SystemExecutionOutput;

use super::UiLabel;
use crate::PickableTexturedRect;
use crate::SpaceKind;
use crate::visualizers::LoadingIndicator;

/// A bounding box produced by a spatial visualizer.
#[derive(Clone, Copy, Debug)]
pub struct SpatialViewBoundingBox {
    pub entity_path_hash: EntityPathHash,
    pub bounding_box: macaw::BoundingBox,

    /// Whether this bounding box is defined in a 2D or 3D subspace.
    ///
    /// If an object can only be defined in a 2D subspace (e.g. a 2D image), this will be `SpaceKind::TwoD`.
    /// Note that such objects can still be placed in a 3D scene, but need a pinhole parent to do so.
    ///
    /// We use this information to filter out 2D objects when computing the overall scene bounding box for a 3D scene,
    /// since the camera plane distance may depend on the scene bounds and including 2D objects would create a feedback loop.
    pub subspace: SpaceKind,
}

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
    bounding_boxes: Vec<SpatialViewBoundingBox>,

    /// Regions of interest for all visualizations, excluding spatial outliers.
    ///
    /// Used for camera framing and other heuristics. For most visualizers this is
    /// identical to the bounding box. Point cloud visualizers may provide a tighter
    /// region that excludes outlier points.
    regions_of_interest: Vec<SpatialViewBoundingBox>,

    /// Textured rectangles that the visualizer produced which can be interacted with.
    pub pickable_rects: Vec<PickableTexturedRect>,
}

impl SpatialViewVisualizerData {
    pub fn add_pickable_rect(&mut self, pickable_rect: PickableTexturedRect, subspace: SpaceKind) {
        self.add_pickable_rect_to_bounding_box(&pickable_rect, subspace);
        self.pickable_rects.push(pickable_rect);
    }

    /// Adds a bounding box and region of interest for an entity.
    ///
    /// For most visualizers these are the same. Use [`Self::add_bounding_box_and_region_of_interest`]
    /// when they differ (e.g. for point clouds with outlier rejection).
    pub fn add_bounding_box_3d(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        self.add_bounding_box(entity, bbox, world_from_obj, SpaceKind::ThreeD);
    }

    pub fn add_bounding_box_2d(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        self.add_bounding_box(entity, bbox, world_from_obj, SpaceKind::TwoD);
    }

    fn add_bounding_box(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
        subspace: SpaceKind,
    ) {
        let transformed = bbox.transform_affine3(&world_from_obj);
        self.bounding_boxes.push(SpatialViewBoundingBox {
            entity_path_hash: entity,
            bounding_box: transformed,
            subspace,
        });
        self.regions_of_interest.push(SpatialViewBoundingBox {
            entity_path_hash: entity,
            bounding_box: transformed,
            subspace,
        });
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
        subspace: SpaceKind,
    ) {
        self.bounding_boxes.push(SpatialViewBoundingBox {
            entity_path_hash: entity,
            bounding_box: bbox.transform_affine3(&world_from_obj),
            subspace,
        });
        self.regions_of_interest.push(SpatialViewBoundingBox {
            entity_path_hash: entity,
            bounding_box: region_of_interest.transform_affine3(&world_from_obj),
            subspace,
        });
    }

    pub fn add_pickable_rect_to_bounding_box(
        &mut self,
        pickable_rect: &PickableTexturedRect,
        subspace: SpaceKind,
    ) {
        let entity_path_hash = pickable_rect.ent_path.hash();
        let bounding_box = pickable_rect.textured_rect.bounding_box();
        self.bounding_boxes.push(SpatialViewBoundingBox {
            entity_path_hash,
            bounding_box,
            subspace,
        });
        self.regions_of_interest.push(SpatialViewBoundingBox {
            entity_path_hash,
            bounding_box,
            subspace,
        });
    }

    pub fn iter_bounding_boxes(&self) -> impl ExactSizeIterator<Item = &SpatialViewBoundingBox> {
        self.bounding_boxes.iter()
    }

    pub fn iter_regions_of_interest(
        &self,
    ) -> impl ExactSizeIterator<Item = &SpatialViewBoundingBox> {
        self.regions_of_interest.iter()
    }
}

/// Iterate over [`SpatialViewVisualizerData`] from all visualizer outputs.
pub fn iter_spatial_data(
    system_output: &SystemExecutionOutput,
) -> impl Iterator<Item = &SpatialViewVisualizerData> {
    system_output
        .visualizer_execution_output
        .per_visualizer
        .values()
        .filter_map(|result| {
            let output = result.as_ref().ok()?;
            output.get_visualizer_data::<SpatialViewVisualizerData>()
        })
}
