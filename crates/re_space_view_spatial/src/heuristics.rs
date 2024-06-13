use egui::NumExt as _;
use nohash_hasher::IntSet;

use re_log_types::EntityPath;
use re_types::SpaceViewClassIdentifier;
use re_viewer_context::ViewerContext;

use crate::{view_kind::SpatialSpaceViewKind, visualizers::SpatialViewVisualizerData};

pub fn auto_size_world_heuristic(
    scene_bbox_accum: &macaw::BoundingBox,
    scene_num_primitives: usize,
) -> f32 {
    if scene_bbox_accum.is_nothing() || scene_bbox_accum.is_nan() {
        return 0.01;
    }

    // Motivation: Size should be proportional to the scene extent, here covered by its diagonal
    let diagonal_length = (scene_bbox_accum.max - scene_bbox_accum.min).length();
    let heuristic0 = diagonal_length * 0.0025;

    // Motivation: A lot of times we look at the entire scene and expect to see everything on a flat screen with some gaps between.
    let size = scene_bbox_accum.size();
    let mut sorted_components = size.to_array();
    sorted_components.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Median is more robust against outlier in one direction (as such pretty poor still though)
    let median_extent = sorted_components[1];
    // sqrt would make more sense but using a smaller root works better in practice.
    let heuristic1 =
        (median_extent / (scene_num_primitives.at_least(1) as f32).powf(1.0 / 1.7)) * 0.25;

    heuristic0.min(heuristic1)
}

/// Returns all entities for which a visualizer of the given kind would be picked.
///
/// I.e. all entities for which at least one visualizer of the specified kind is applicable
/// *and* has a matching indicator component.
pub fn default_visualized_entities_for_visualizer_kind(
    ctx: &ViewerContext<'_>,
    space_view_class_identifier: SpaceViewClassIdentifier,
    visualizer_kind: SpatialSpaceViewKind,
) -> IntSet<EntityPath> {
    re_tracing::profile_function!();

    ctx.space_view_class_registry
        .new_visualizer_collection(space_view_class_identifier)
        .iter_with_identifiers()
        .filter_map(|(id, visualizer)| {
            let data = visualizer
                .data()?
                .downcast_ref::<SpatialViewVisualizerData>()?;

            if data.preferred_view_kind == Some(visualizer_kind) {
                let indicator_matching = ctx.indicated_entities_per_visualizer.get(&id)?;
                let applicable = ctx.applicable_entities_per_visualizer.get(&id)?;
                Some(indicator_matching.intersection(applicable))
            } else {
                None
            }
        })
        .flatten()
        .cloned()
        .collect()
}
