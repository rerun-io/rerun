use nohash_hasher::IntSet;

use re_log_types::EntityPath;
use re_types::ViewClassIdentifier;
use re_viewer_context::ViewerContext;

use crate::{view_kind::SpatialSpaceViewKind, visualizers::SpatialViewVisualizerData};

/// Returns all entities for which a visualizer of the given kind would be picked.
///
/// I.e. all entities for which at least one visualizer of the specified kind is applicable
/// *and* has a matching indicator component.
pub fn default_visualized_entities_for_visualizer_kind(
    ctx: &ViewerContext<'_>,
    space_view_class_identifier: ViewClassIdentifier,
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
