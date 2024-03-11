use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_viewer_context::{ApplicableEntities, PerVisualizer, SpaceViewClass, VisualizableEntities};

/// Determines the set of visible entities for a given space view.
// TODO(andreas): This should be part of the SpaceView's (non-blueprint) state.
// Updated whenever `applicable_entities_per_visualizer` or the space view blueprint changes.
pub fn determine_visualizable_entities(
    applicable_entities_per_visualizer: &PerVisualizer<ApplicableEntities>,
    entity_db: &EntityDb,
    visualizers: &re_viewer_context::VisualizerCollection,
    class: &dyn SpaceViewClass,
    space_origin: &EntityPath,
) -> PerVisualizer<VisualizableEntities> {
    re_tracing::profile_function!();

    let filter_ctx = class.visualizable_filter_context(space_origin, entity_db);

    PerVisualizer::<VisualizableEntities>(
        visualizers
            .iter_with_identifiers()
            .map(|(visualizer_identifier, visualizer_system)| {
                let entities = if let Some(applicable_entities) =
                    applicable_entities_per_visualizer.get(&visualizer_identifier)
                {
                    visualizer_system.filter_visualizable_entities(
                        applicable_entities.clone(),
                        filter_ctx.as_ref(),
                    )
                } else {
                    VisualizableEntities::default()
                };

                (visualizer_identifier, entities)
            })
            .collect(),
    )
}
