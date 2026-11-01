//! Shared component drag-and-drop handling for time series and state timeline views.

use re_log_types::{ComponentPath, ResolvedEntityPathRule, RuleEffect};
use re_sdk_types::Archetype as _;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::archetypes::{ActiveVisualizers, ViewContents};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_viewer_context::ComponentMatch;
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

/// Compute component drop feedback and, once `released`, persist the selected mappings.
pub fn handle_component_drop(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    component_paths: &[ComponentPath],
    released: bool,
    candidates: &[(ViewSystemIdentifier, ComponentIdentifier)],
) -> ComponentDropResult {
    ComponentDropHandler {
        ctx,
        view_id,
        candidates,
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

    /// Visualizers and their required slots, in tie-breaking preference order.
    candidates: &'a [(ViewSystemIdentifier, ComponentIdentifier)],
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
            self.apply_component_drop(component_paths);
        }

        ComponentDropResult::Accept
    }

    fn compatible_mapping(
        &self,
        component_path: &ComponentPath,
    ) -> Option<(ViewSystemIdentifier, RecommendedMappings)> {
        /// Declaration order ranks identity mappings (including native semantic matches)
        /// before heuristic mappings, then physical datatype matches.
        /// `min_by_key` selects the lowest rank, with candidate order breaking ties.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum MappingPriority {
            Identity,
            Heuristic,
            PhysicalDatatype,
        }

        self.candidates
            .iter()
            .filter_map(|&(visualizer, target_component)| {
                let reason = self
                    .ctx
                    .visualizable_entities_per_visualizer
                    .get(&visualizer)?
                    .get(&component_path.entity_path)?;
                let (priority, mappings) = match reason {
                    VisualizableReason::Always | VisualizableReason::ExactMatchAny => (
                        MappingPriority::Identity,
                        RecommendedMappings::new(
                            target_component,
                            VisualizerComponentSource::identity(*component_path.component()),
                        ),
                    ),
                    VisualizableReason::SingleRequiredComponentMatch(m) => {
                        match m.matches.get(component_path.component())? {
                            ComponentMatch::NativeSemantics { .. } => (
                                MappingPriority::Identity,
                                RecommendedMappings::new(
                                    target_component,
                                    VisualizerComponentSource::identity(
                                        *component_path.component(),
                                    ),
                                ),
                            ),
                            ComponentMatch::HeuristicMapping { mappings, .. } => {
                                (MappingPriority::Heuristic, mappings.clone())
                            }
                            ComponentMatch::PhysicalDatatypeOnly {
                                field_accessors, ..
                            } => (
                                MappingPriority::PhysicalDatatype,
                                RecommendedMappings::new(
                                    target_component,
                                    VisualizerComponentSource::SourceComponent {
                                        source_component: *component_path.component(),
                                        // TODO(andreas): Offer a choice when several fields match.
                                        selector: field_accessors
                                            .first()
                                            .map_or(String::new(), |(selector, _datatype)| {
                                                selector.to_string()
                                            }),
                                    },
                                ),
                            ),
                        }
                    }
                    // A single dropped component cannot supply both buffer and format.
                    VisualizableReason::BufferAndFormatMatch(_) => return None,
                };
                Some((priority, visualizer, mappings))
            })
            .min_by_key(|(priority, _, _)| *priority)
            .map(|(_, visualizer, mappings)| (visualizer, mappings))
    }

    /// Drop outcome for a single `component_path`.
    fn can_drop_component(&self, component_path: &ComponentPath) -> ComponentDropResult {
        let Some((visualizer, mappings)) = self.compatible_mapping(component_path) else {
            return ComponentDropResult::Incompatible;
        };
        self.drop_result_for_mapping(component_path, visualizer, &mappings)
    }

    fn drop_result_for_mapping(
        &self,
        component_path: &ComponentPath,
        visualizer: ViewSystemIdentifier,
        mappings: &RecommendedMappings,
    ) -> ComponentDropResult {
        let existing_instructions = self
            .ctx
            .lookup_query_result(self.view_id)
            .tree
            .lookup_result_by_path(component_path.entity_path.hash())
            .map(|data_result| data_result.visualizer_instructions.clone())
            .unwrap_or_default();

        let already_visualized = existing_instructions.iter().any(|v| {
            v.visualizer_type == visualizer && mappings.is_covered_by(&v.component_mappings)
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
            .filter_map(|cp| self.compatible_mapping(cp).map(|mapping| (mapping, cp)))
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

        for ((visualizer, mappings), component_path) in compatible {
            self.add_visualizer_for_mapping(visualizer, mappings, component_path);
        }
    }

    fn add_visualizer_for_mapping(
        &self,
        visualizer: ViewSystemIdentifier,
        mappings: RecommendedMappings,
        component_path: &ComponentPath,
    ) {
        // Skip if an equivalent visualizer already exists.
        if self.drop_result_for_mapping(component_path, visualizer, &mappings)
            != ComponentDropResult::Accept
        {
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

        let new_instruction = mappings.into_visualizer_instruction(
            VisualizerInstructionId::new_random(),
            visualizer,
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
