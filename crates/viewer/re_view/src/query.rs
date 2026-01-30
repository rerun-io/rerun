use nohash_hasher::{IntMap, IntSet};
use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::hash::Hash64;
use re_log_types::{TimeInt, TimelineName};
use re_query::LatestAtResults;
use re_sdk_types::blueprint::datatypes::ComponentSourceKind;
use re_types_core::{Archetype, ComponentIdentifier};
use re_viewer_context::{DataResult, QueryRange, ViewContext, ViewQuery, ViewerContext};

use crate::HybridResults;
use crate::results_ext::{HybridLatestAtResults, HybridRangeResults};

// ---

/// Queries for the given `components` using range semantics with blueprint support.
///
/// Data will be resolved, in order of priority:
/// - Data overrides from the blueprint
/// - Data from the recording
/// - Default data from the blueprint
/// - Fallback from the visualizer
/// - Placeholder from the component.
///
/// Data should be accessed via the [`crate::RangeResultsExt`] trait which is implemented for
/// [`crate::HybridResults`].
pub fn range_with_blueprint_resolved_data<'a>(
    ctx: &ViewContext<'a>,
    _annotations: Option<&re_viewer_context::Annotations>,
    range_query: &RangeQuery,
    data_result: &re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
) -> HybridRangeResults<'a> {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    // TODO(andreas): It would be great to avoid querying for overrides & store values that aren't used due to explicit source components.
    // Logic gets surprisingly complicated quickly though.

    let mut components = components.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(
        ctx.viewer_ctx,
        visualizer_instruction,
        components.iter().copied(),
    );

    // Apply component mappings when querying the recording.
    let mut active_remappings = Vec::new();
    let mut component_sources = IntMap::default();
    let store_results = {
        // Apply component mappings when querying the recording.]
        for (target, source) in &visualizer_instruction.component_mappings {
            component_sources.insert(*target, Ok(source.source_kind()));

            if !source.is_identity_mapping(*target)
                && let re_viewer_context::VisualizerComponentSource::SourceComponent {
                    source_component,
                    selector: _, // TODO(RR-3308): implement selector logic
                } = source
                && components.remove(target)
            {
                components.insert(*source_component);
                active_remappings.push((*target, *source_component));
            }
        }

        let mut results = ctx.recording_engine().cache().range(
            range_query,
            &data_result.entity_path,
            components.iter().copied(),
        );

        // Apply mapping to the results.
        for (target, selector) in &active_remappings {
            if let Some(mut chunks) = results.components.remove(selector) {
                for chunk in &mut chunks {
                    *chunk = chunk.with_renamed_component(*selector, *target);
                }

                results.components.insert(*target, chunks);
            }
        }

        results
    };

    // Auto-determine remaining mapping sources.
    #[expect(clippy::iter_over_hash_type)] // Doing that to fill another hashmap.
    for component in &components {
        match component_sources.entry(*component) {
            std::collections::hash_map::Entry::Occupied(_) => {}
            std::collections::hash_map::Entry::Vacant(entry) => {
                let source = if overrides.get(*component).is_some() {
                    ComponentSourceKind::Override
                } else if store_results.components.contains_key(component) {
                    ComponentSourceKind::SourceComponent
                } else {
                    ComponentSourceKind::Default
                };

                entry.insert(Ok(source));
            }
        }
    }

    HybridRangeResults {
        overrides,
        store_results,
        view_defaults: &ctx.query_result.view_defaults,
        component_sources,
        component_mappings_hash: Hash64::hash(&active_remappings),
    }
}

/// Queries for the given `components` using latest-at semantics with blueprint support.
///
/// Data will be resolved, in order of priority:
/// - Data overrides from the blueprint
/// - Data from the recording
/// - Default data from the blueprint
/// - Fallback from the visualizer
/// - Placeholder from the component.
///
/// Data should be accessed via the [`crate::RangeResultsExt`] trait which is implemented for
/// [`crate::HybridResults`].
pub fn latest_at_with_blueprint_resolved_data<'a>(
    ctx: &'a ViewContext<'a>,
    _annotations: Option<&'a re_viewer_context::Annotations>,
    latest_at_query: &LatestAtQuery,
    data_result: &'a re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
) -> HybridLatestAtResults<'a> {
    // This is called very frequently, don't put a profile scope here.

    // TODO(andreas): It would be great to avoid querying for overrides & store values that aren't used due to explicit source components.
    // Logic gets surprisingly complicated quickly though.

    let mut components = components.into_iter().collect::<IntSet<_>>();
    let overrides = if let Some(visualizer_instruction) = visualizer_instruction {
        query_overrides(
            ctx.viewer_ctx,
            visualizer_instruction,
            components.iter().copied(),
        )
    } else {
        query_overrides_at_path(
            ctx.viewer_ctx,
            data_result.override_base_path(),
            components.iter().copied(),
        )
    };

    // Apply component mappings when querying the recording.
    let mut active_remappings = Vec::new();
    let mut component_sources = IntMap::default();
    if let Some(visualizer_instruction) = visualizer_instruction {
        for (target, source) in &visualizer_instruction.component_mappings {
            component_sources.insert(*target, Ok(source.source_kind()));

            if !source.is_identity_mapping(*target)
                && let re_viewer_context::VisualizerComponentSource::SourceComponent {
                    source_component,
                    selector: _, // TODO(RR-3308): implement selector logic
                } = source
                && components.remove(target)
            {
                components.insert(*source_component);
                active_remappings.push((*target, *source_component));
            }
        }
    }

    let mut store_results = ctx.viewer_ctx.recording_engine().cache().latest_at(
        latest_at_query,
        &data_result.entity_path,
        components.iter().copied(),
    );

    // Apply mapping to the results.
    for (target, selector) in &active_remappings {
        if let Some(chunk) = store_results.components.remove(selector) {
            let chunk = std::sync::Arc::new(chunk.with_renamed_component(*selector, *target))
                .to_unit()
                .expect("The source chunk was a unit chunk.");
            store_results.components.insert(*target, chunk);
        }
    }

    // Auto-determine remaining mapping sources.
    #[expect(clippy::iter_over_hash_type)] // Doing that to fill another hashmap.
    for component in &components {
        match component_sources.entry(*component) {
            std::collections::hash_map::Entry::Occupied(_) => {}
            std::collections::hash_map::Entry::Vacant(entry) => {
                let source = if overrides.get(*component).is_some() {
                    ComponentSourceKind::Override
                } else if store_results.components.contains_key(component) {
                    ComponentSourceKind::SourceComponent
                } else {
                    ComponentSourceKind::Default
                };

                entry.insert(Ok(source));
            }
        }
    }

    HybridLatestAtResults {
        overrides,
        store_results,
        view_defaults: &ctx.query_result.view_defaults,
        ctx,
        query: latest_at_query.clone(),
        data_result,
        component_sources,
        component_indices_hash: Hash64::hash(&active_remappings),
    }
}

pub fn query_archetype_with_history<'a>(
    ctx: &'a ViewContext<'a>,
    timeline: &TimelineName,
    timeline_cursor: TimeInt,
    query_range: &QueryRange,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    data_result: &'a re_viewer_context::DataResult,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
) -> HybridResults<'a> {
    match query_range {
        QueryRange::TimeRange(time_range) => {
            let range_query = RangeQuery::new(
                *timeline,
                re_log_types::AbsoluteTimeRange::from_relative_time_range(
                    time_range,
                    timeline_cursor,
                ),
            );
            let results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &range_query,
                data_result,
                components,
                visualizer_instruction,
            );
            (range_query, results).into()
        }
        QueryRange::LatestAt => {
            let latest_query = LatestAtQuery::new(*timeline, timeline_cursor);
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_query,
                data_result,
                components,
                Some(visualizer_instruction),
            );
            (latest_query, results).into()
        }
    }
}

fn query_overrides(
    ctx: &ViewerContext<'_>,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    components: impl IntoIterator<Item = ComponentIdentifier>,
) -> LatestAtResults {
    if visualizer_instruction.component_overrides.is_empty() {
        LatestAtResults::empty("<overrides>".into(), ctx.current_query())
    } else {
        query_overrides_at_path(
            ctx,
            &visualizer_instruction.override_path,
            components
                .into_iter()
                .filter(|c| visualizer_instruction.component_overrides.contains(c)),
        )
    }
}

fn query_overrides_at_path(
    ctx: &ViewerContext<'_>,
    blueprint_path: &re_log_types::EntityPath,
    components: impl IntoIterator<Item = ComponentIdentifier>,
) -> LatestAtResults {
    // First see if any components have overrides.
    let mut overrides = LatestAtResults::empty("<overrides>".into(), ctx.current_query());

    let blueprint_engine = &ctx.store_context.blueprint.storage_engine();

    for component in components {
        // TODO(andreas): Batch these queries?
        let component_override_result =
            blueprint_engine
                .cache()
                .latest_at(ctx.blueprint_query, blueprint_path, [component]);

        // If we successfully find a non-empty override, add it to our results.
        if let Some(value) = component_override_result.get(component) {
            let index = value.index(&ctx.blueprint_query.timeline());

            // NOTE: This can never happen, but I'd rather it happens than an unwrap.
            debug_assert!(index.is_some(), "{value:#?}");
            let index = index.unwrap_or((TimeInt::STATIC, RowId::ZERO));

            overrides.add(component, index, value.clone());
        }
    }
    overrides
}

pub trait DataResultQuery {
    fn latest_at_with_blueprint_resolved_data<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> HybridLatestAtResults<'a>;

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> HybridLatestAtResults<'a>;

    /// Queries for the given components, taking into account:
    /// * visible history if enabled
    /// * blueprint overrides & defaults
    fn query_components_with_history<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        component_descriptors: impl IntoIterator<Item = ComponentIdentifier>,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> HybridResults<'a>;

    /// Queries for all components of an archetype, taking into account:
    /// * visible history if enabled
    /// * blueprint overrides & defaults
    fn query_archetype_with_history<'a, A: Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> HybridResults<'a> {
        self.query_components_with_history(
            ctx,
            view_query,
            A::all_component_identifiers(),
            visualizer_instruction,
        )
    }
}

impl DataResultQuery for DataResult {
    fn latest_at_with_blueprint_resolved_data<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> HybridLatestAtResults<'a> {
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            A::all_component_identifiers(),
            visualizer_instruction,
        )
    }

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
        visualizer_instruction: Option<&re_viewer_context::VisualizerInstruction>,
    ) -> HybridLatestAtResults<'a> {
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            std::iter::once(component),
            visualizer_instruction,
        )
    }

    fn query_components_with_history<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        components: impl IntoIterator<Item = ComponentIdentifier>,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> HybridResults<'a> {
        query_archetype_with_history(
            ctx,
            &view_query.timeline,
            view_query.latest_at,
            self.query_range(),
            components,
            self,
            visualizer_instruction,
        )
    }
}
