use re_log_types::{ComponentPath, EntityPath, ResolvedEntityPathRule, RuleEffect};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::StateChange;
use re_sdk_types::blueprint::archetypes::{ActiveVisualizers, ViewContents};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_viewer_context::{
    BlueprintContext as _, IdentifiedViewSystem as _, RecommendedMappings, ViewId, ViewerContext,
    VisualizerComponentSource, VisualizerInstruction,
};
use re_viewport_blueprint::ViewBlueprint;

use crate::StateVisualizer;

/// For each dropped component, add a `StateVisualizer` instruction whose
/// `StateChange.state` is mapped from the dropped component.
///
/// If a component's entity is not yet matched by the view's entity path filter,
/// add an including-subtree rule so the new instruction is reachable.
pub fn handle_component_drop(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    component_paths: &[ComponentPath],
) {
    let Some(view_blueprint) =
        ViewBlueprint::try_from_db(view_id, ctx.blueprint_db(), ctx.blueprint_query)
    else {
        return;
    };

    let current_filter = view_blueprint.contents.entity_path_filter();
    let missing_entities: Vec<EntityPath> = component_paths
        .iter()
        .map(|cp| cp.entity_path.clone())
        .filter(|entity| !current_filter.matches(entity))
        .collect();

    if !missing_entities.is_empty() {
        view_blueprint
            .contents
            .mutate_entity_path_filter(ctx, |filter| {
                for entity in &missing_entities {
                    filter.add_rule(
                        RuleEffect::Include,
                        ResolvedEntityPathRule::including_subtree(entity),
                    );
                }
            });
    }

    for component_path in component_paths {
        add_state_visualizer_for_component(ctx, view_id, component_path);
    }
}

fn add_state_visualizer_for_component(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    component_path: &ComponentPath,
) {
    let entity_path = &component_path.entity_path;
    let override_base_path =
        ViewContents::blueprint_base_visualizer_path_for_entity(view_id.uuid(), entity_path);

    let query_result = ctx.lookup_query_result(view_id);
    let existing_instructions: Vec<VisualizerInstruction> = query_result
        .tree
        .lookup_result_by_path(entity_path.hash())
        .map(|data_result| data_result.visualizer_instructions.clone())
        .unwrap_or_default();

    let recommended_mappings = RecommendedMappings::new(
        StateChange::descriptor_state().component,
        VisualizerComponentSource::SourceComponent {
            source_component: component_path.component,
            selector: String::new(),
        },
    );

    // Skip if an equivalent visualizer already exists.
    if existing_instructions.iter().any(|v| {
        v.visualizer_type == StateVisualizer::identifier()
            && recommended_mappings.is_covered_by(&v.component_mappings)
    }) {
        return;
    }

    let new_instruction = recommended_mappings.into_visualizer_instruction(
        VisualizerInstructionId::new_random(),
        StateVisualizer::identifier(),
        &override_base_path,
    );

    let active_visualizer_archetype = ActiveVisualizers::new(
        existing_instructions
            .iter()
            .map(|v| &v.id)
            .chain(std::iter::once(&new_instruction.id))
            .map(|v| v.0),
    );

    // If this is the first time we persist ActiveVisualizers for this entity,
    // also write out the previously-heuristic instructions so they survive.
    let did_not_yet_persist = ctx
        .blueprint_db()
        .latest_at(
            ctx.blueprint_query,
            &override_base_path,
            ActiveVisualizers::all_components()
                .iter()
                .map(|c| c.component),
        )
        .components
        .is_empty();
    if did_not_yet_persist {
        for instruction in &existing_instructions {
            instruction.write_instruction_to_blueprint(ctx);
        }
    }

    ctx.save_blueprint_archetype(override_base_path, &active_visualizer_archetype);
    new_instruction.write_instruction_to_blueprint(ctx);
}
