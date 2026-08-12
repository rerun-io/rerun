use re_log_types::EntityPathHash;
use re_renderer::RobustBounds;
use re_viewer_context::SystemExecutionOutput;

use super::UiLabel;
use crate::PickableTexturedRect;
use crate::SpaceKind;
use crate::visualizers::LoadingIndicator;

/// The bounds of something a spatial visualizer showed.
#[derive(Clone, Copy, Debug)]
pub struct SpatialViewBounds {
    pub entity_path_hash: EntityPathHash,

    /// The exact bounding box, plus a region of interest that excludes spatial outliers.
    ///
    /// The region of interest is used for camera framing and other heuristics.
    /// For most visualizers it is identical to the bounding box.
    /// Point cloud visualizers estimate it statistically, so it may be either
    /// smaller or larger than the bounding box.
    pub bounds: RobustBounds,

    /// Whether these bounds are defined in a 2D or 3D subspace.
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

    /// Bounds of all visualizations that the visualizer showed.
    bounds: Vec<SpatialViewBounds>,

    /// Textured rectangles that the visualizer produced which can be interacted with.
    pub pickable_rects: Vec<PickableTexturedRect>,
}

impl SpatialViewVisualizerData {
    pub fn add_pickable_rect(&mut self, pickable_rect: PickableTexturedRect, subspace: SpaceKind) {
        self.add_pickable_rect_to_bounding_box(&pickable_rect, subspace);
        self.pickable_rects.push(pickable_rect);
    }

    /// Adds a bounding box for an entity, with no outlier rejection.
    ///
    /// The region of interest becomes the bounding box itself. Use [`Self::add_bounds`]
    /// when they differ (e.g. for point clouds with outlier rejection).
    pub fn add_bounding_box_3d(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        self.add_bounds(
            entity,
            RobustBounds::from_bbox(bbox),
            world_from_obj,
            SpaceKind::ThreeD,
        );
    }

    /// Adds a bounding box for an entity, with no outlier rejection.
    ///
    /// The region of interest becomes the bounding box itself. Use [`Self::add_bounds`]
    /// when they differ (e.g. for point clouds with outlier rejection).
    pub fn add_bounding_box_2d(
        &mut self,
        entity: EntityPathHash,
        bbox: macaw::BoundingBox,
        world_from_obj: glam::Affine3A,
    ) {
        self.add_bounds(
            entity,
            RobustBounds::from_bbox(bbox),
            world_from_obj,
            SpaceKind::TwoD,
        );
    }

    /// Adds the bounds of an entity, given in object space.
    pub fn add_bounds(
        &mut self,
        entity: EntityPathHash,
        bounds: RobustBounds,
        world_from_obj: glam::Affine3A,
        subspace: SpaceKind,
    ) {
        self.bounds.push(SpatialViewBounds {
            entity_path_hash: entity,
            bounds: bounds.transform_affine3(&world_from_obj),
            subspace,
        });
    }

    pub fn add_pickable_rect_to_bounding_box(
        &mut self,
        pickable_rect: &PickableTexturedRect,
        subspace: SpaceKind,
    ) {
        self.bounds.push(SpatialViewBounds {
            entity_path_hash: pickable_rect.ent_path.hash(),
            bounds: RobustBounds::from_bbox(pickable_rect.textured_rect.bounding_box()),
            subspace,
        });
    }

    pub fn iter_bounds(&self) -> impl ExactSizeIterator<Item = &SpatialViewBounds> {
        self.bounds.iter()
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
