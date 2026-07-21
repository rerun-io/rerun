//! Shared component drag-and-drop handling for views that map a dropped component onto a single
//! visualizer slot (e.g. the time series and state timeline views).

use re_log_types::{ComponentPath, ResolvedEntityPathRule, RuleEffect};
use re_sdk_types::Archetype as _;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::archetypes::{ActiveVisualizers, ViewContents};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_viewer_context::DatatypeMatch;
use re_viewer_context::{
    BlueprintContext as _, RecommendedMappings, ViewId, ViewSystemIdentifier, ViewerContext,
    VisualizableReason, VisualizerComponentSource, VisualizerInstruction,
};
use re_viewport_blueprint::ViewBlueprint;

/// Outcome of dropping (or hovering) components onto a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentDropResult {
    /// At least one component can be dropped.
    Accept,

    /// Every compatible component is already visualized, so the drop would be a no-op.
    CompatibleButAlreadyVisualized,

    /// None of the components is a compatible source for the view's visualizer.
    Incompatible,
}

/// Drive `ViewClass::handle_component_drop` for a view that maps a dropped component onto a single
/// visualizer slot: compute the drop outcome and, once `released`, perform the mutation.
///
/// For each dropped component a `visualizer` instruction is added whose `target_component` slot
/// (e.g. `Scalars.scalars`) is remapped from it.
pub fn handle_component_drop(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    component_paths: &[ComponentPath],
    released: bool,
    visualizer: ViewSystemIdentifier,
    target_component: ComponentIdentifier,
) -> ComponentDropResult {
    ComponentDropHandler {
        ctx,
        view_id,
        visualizer,
        target_component,
    }
    .on_component_drop(component_paths, released)
}

/// Configures how a view accepts dropped components and turns them into visualizer instructions.
#[derive(Clone, Copy)]
struct ComponentDropHandler<'a> {
    /// Context the drop is handled against.
    ctx: &'a ViewerContext<'a>,

    /// View receiving the drop.
    view_id: ViewId,

    /// Visualizer added for each dropped component.
    visualizer: ViewSystemIdentifier,

    /// The component slot that a dropped component is remapped into (e.g. `Scalars.scalars`).
    target_component: ComponentIdentifier,
}

impl ComponentDropHandler<'_> {
    fn on_component_drop(
        &self,
        component_paths: &[ComponentPath],
        released: bool,
    ) -> ComponentDropResult {
        let outcome = self.can_drop_any_component(component_paths);
        if outcome != ComponentDropResult::Accept {
            return outcome;
        }

        if released {
            egui::DragAndDrop::clear_payload(self.ctx.egui_ctx());
            self.apply_component_drop(component_paths);
        }

        ComponentDropResult::Accept
    }

    /// If `component_path` is a compatible source for the configured visualizer, returns a component source for the visualizer instruction.
    fn compatible_component_source(
        &self,
        component_path: &ComponentPath,
    ) -> Option<VisualizerComponentSource> {
        let visualizable_entities = self
            .ctx
            .visualizable_entities_per_visualizer
            .get(&self.visualizer)?;

        match visualizable_entities.get(&component_path.entity_path) {
            Some(VisualizableReason::Always | VisualizableReason::ExactMatchAny) => Some(
                VisualizerComponentSource::identity(*component_path.component()),
            ),

            Some(VisualizableReason::SingleRequiredComponentMatch(m)) => {
                let component_match = m.matches.get(&component_path.component)?;

                let selector = match component_match {
                    // TODO(andreas): Picking just the first here is a stop gap measure.
                    // We should be asking the view for a good recommendation or just show all of them?
                    DatatypeMatch::PhysicalDatatypeOnly { selectors, .. } => selectors
                        .first()
                        .map_or(String::new(), |(selector, _datatype)| selector.to_string()),

                    DatatypeMatch::NativeSemantics { .. } => String::new(),
                };

                Some(VisualizerComponentSource::SourceComponent {
                    source_component: *component_path.component(),
                    selector,
                })
            }

            // A single component can't satisfy a buffer + format match because that would require two.
            Some(VisualizableReason::BufferAndFormatMatch(_)) | None => None,
        }
    }

    /// Drop outcome for a single `component_path`.
    fn can_drop_component(&self, component_path: &ComponentPath) -> ComponentDropResult {
        let Some(component_source) = self.compatible_component_source(component_path) else {
            return ComponentDropResult::Incompatible;
        };
        self.drop_result_for_source(component_path, &component_source)
    }

    /// Drop outcome given the already-extracted compatible `source` for `component_path`.
    fn drop_result_for_source(
        &self,
        component_path: &ComponentPath,
        source: &VisualizerComponentSource,
    ) -> ComponentDropResult {
        let existing_instructions = self
            .ctx
            .lookup_query_result(self.view_id)
            .tree
            .lookup_result_by_path(component_path.entity_path.hash())
            .map(|data_result| data_result.visualizer_instructions.clone())
            .unwrap_or_default();

        let recommended_mappings = RecommendedMappings::new(self.target_component, source.clone());

        let already_visualized = existing_instructions.iter().any(|v| {
            v.visualizer_type == self.visualizer
                && recommended_mappings.is_covered_by(&v.component_mappings)
        });
        if already_visualized {
            return ComponentDropResult::CompatibleButAlreadyVisualized;
        }

        ComponentDropResult::Accept
    }

    /// Drop outcome for dropping `component_paths` onto the view.
    fn can_drop_any_component(&self, component_paths: &[ComponentPath]) -> ComponentDropResult {
        let mut has_compatible_but_already_visualized = false;
        for cp in component_paths {
            match self.can_drop_component(cp) {
                ComponentDropResult::Accept => return ComponentDropResult::Accept,
                // At least one compatible component exists but is already visualized: more
                // informative than the generic incompatible outcome, so let it take precedence.
                ComponentDropResult::CompatibleButAlreadyVisualized => {
                    has_compatible_but_already_visualized = true;
                }
                ComponentDropResult::Incompatible => {}
            }
        }

        if has_compatible_but_already_visualized {
            return ComponentDropResult::CompatibleButAlreadyVisualized;
        }

        ComponentDropResult::Incompatible
    }

    /// For each compatible dropped component, add a visualizer instruction whose target slot is
    /// mapped from the dropped component.
    ///
    /// If a component's entity is not yet matched by the view's entity path filter, add an
    /// including-subtree rule so the new instruction is reachable.
    fn apply_component_drop(&self, component_paths: &[ComponentPath]) {
        let Some(view_blueprint) = ViewBlueprint::try_from_db(
            self.view_id,
            self.ctx.blueprint_db(),
            self.ctx.blueprint_query,
        ) else {
            return;
        };

        // Only act on compatible components; others can't be visualized by this view.
        let compatible: Vec<(_, _)> = component_paths
            .iter()
            .filter_map(|cp| {
                self.compatible_component_source(cp)
                    .map(|source| (source, cp))
            })
            .collect();

        let current_filter = view_blueprint.contents.entity_path_filter();
        let missing_entities: Vec<_> = compatible
            .iter()
            .map(|(_, cp)| cp.entity_path())
            .filter(|entity| !current_filter.matches(entity))
            .collect();

        if !missing_entities.is_empty() {
            view_blueprint
                .contents
                .mutate_entity_path_filter(self.ctx, |filter| {
                    for entity in &missing_entities {
                        filter.add_rule(
                            RuleEffect::Include,
                            ResolvedEntityPathRule::including_subtree(entity),
                        );
                    }
                });
        }

        for (source, component_path) in compatible {
            self.add_visualizer_for_component_source(source, component_path);
        }
    }

    fn add_visualizer_for_component_source(
        &self,
        source: VisualizerComponentSource,
        component_path: &ComponentPath,
    ) {
        // Skip if an equivalent visualizer already exists.
        if self.drop_result_for_source(component_path, &source) != ComponentDropResult::Accept {
            return;
        }

        let entity_path = &component_path.entity_path;
        let override_base_path = ViewContents::blueprint_base_visualizer_path_for_entity(
            self.view_id.uuid(),
            entity_path,
        );

        let query_result = self.ctx.lookup_query_result(self.view_id);
        let existing_instructions: Vec<VisualizerInstruction> = query_result
            .tree
            .lookup_result_by_path(entity_path.hash())
            .map(|data_result| data_result.visualizer_instructions.clone())
            .unwrap_or_default();

        let recommended_mappings = RecommendedMappings::new(self.target_component, source);

        let new_instruction = recommended_mappings.into_visualizer_instruction(
            VisualizerInstructionId::new_random(),
            self.visualizer,
            &override_base_path,
        );

        let active_visualizer_archetype = ActiveVisualizers::new(
            std::iter::chain(
                existing_instructions.iter().map(|v| &v.id),
                std::iter::once(&new_instruction.id),
            )
            .map(|v| v.0),
        );

        // If this is the first time we persist ActiveVisualizers for this entity,
        // also write out the previously-heuristic instructions so they survive.
        let did_not_yet_persist = self
            .ctx
            .blueprint_db()
            .latest_at(
                self.ctx.blueprint_query,
                &override_base_path,
                ActiveVisualizers::all_components()
                    .iter()
                    .map(|c| c.component),
            )
            .components
            .is_empty();
        if did_not_yet_persist {
            for instruction in &existing_instructions {
                instruction.write_instruction_to_blueprint(self.ctx);
            }
        }

        self.ctx
            .save_blueprint_archetype(override_base_path, &active_visualizer_archetype);
        new_instruction.write_instruction_to_blueprint(self.ctx);
    }
}
