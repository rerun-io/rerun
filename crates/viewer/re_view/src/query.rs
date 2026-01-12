use nohash_hasher::IntSet;
use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::hash::Hash64;
use re_log_types::{TimeInt, TimelineName};
use re_query::LatestAtResults;
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

    let mut components = components.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(
        ctx.viewer_ctx,
        visualizer_instruction,
        components.iter().copied(),
    );

    // No need to query for components that have overrides.
    components.retain(|component| overrides.get(*component).is_none());

    let results = {
        // Apply component mappings when querying the recording.
        for mapping in &visualizer_instruction.component_mappings {
            if components.remove(&mapping.target) {
                components.insert(mapping.selector);
            }
        }

        let mut results =
            ctx.recording_engine()
                .cache()
                .range(range_query, &data_result.entity_path, components);

        // Apply mapping to the results.
        for mapping in &visualizer_instruction.component_mappings {
            if let Some(mut chunks) = results.components.remove(&mapping.selector) {
                for chunk in &mut chunks {
                    *chunk = chunk.with_renamed_component(mapping.selector, mapping.target);
                }

                results.components.insert(mapping.target, chunks);
            }
        }

        results
    };

    HybridRangeResults {
        overrides,
        results,
        defaults: &ctx.query_result.component_defaults,
        component_mappings_hash: Hash64::hash(&visualizer_instruction.component_mappings),
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
///
/// If `query_shadowed_components` is true, store components will be queried, even if they are not used.
pub fn latest_at_with_blueprint_resolved_data<'a>(
    ctx: &'a ViewContext<'a>,
    _annotations: Option<&'a re_viewer_context::Annotations>,
    latest_at_query: &LatestAtQuery,
    data_result: &'a re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    query_shadowed_components: bool,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
) -> HybridLatestAtResults<'a> {
    // This is called very frequently, don't put a profile scope here.

    let mut components = components.into_iter().collect::<IntSet<_>>();
    let overrides = query_overrides(
        ctx.viewer_ctx,
        visualizer_instruction,
        components.iter().copied(),
    );

    // No need to query for components that have overrides unless opted in!
    if !query_shadowed_components {
        components.retain(|component| overrides.get(*component).is_none());
    }

    let results = {
        // Apply component mappings when querying the recording.
        for mapping in &visualizer_instruction.component_mappings {
            if components.remove(&mapping.target) {
                components.insert(mapping.selector);
            }
        }
        let mut results = ctx.viewer_ctx.recording_engine().cache().latest_at(
            latest_at_query,
            &data_result.entity_path,
            components,
        );

        // Apply mapping to the results.
        for mapping in &visualizer_instruction.component_mappings {
            if let Some(chunk) = results.components.remove(&mapping.selector) {
                let chunk = std::sync::Arc::new(
                    chunk.with_renamed_component(mapping.selector, mapping.target),
                )
                .to_unit()
                .expect("The source chunk was a unit chunk.");
                results.components.insert(mapping.target, chunk);
            }
        }

        results
    };

    HybridLatestAtResults {
        overrides,
        results,
        defaults: &ctx.query_result.component_defaults,
        ctx,
        query: latest_at_query.clone(),
        data_result,
        component_mappings_hash: Hash64::hash(&visualizer_instruction.component_mappings),
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
            let query_shadowed_defaults = false;
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_query,
                data_result,
                components,
                query_shadowed_defaults,
                visualizer_instruction,
            );
            (latest_query, results).into()
        }
    }
}

pub fn query_overrides(
    ctx: &ViewerContext<'_>,
    visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    components: impl IntoIterator<Item = ComponentIdentifier>,
) -> LatestAtResults {
    // First see if any components have overrides.
    let mut overrides = LatestAtResults::empty("<overrides>".into(), ctx.current_query());

    let blueprint_engine = &ctx.store_context.blueprint.storage_engine();

    for component in components {
        if visualizer_instruction
            .component_overrides
            .contains(&component)
        {
            let component_override_result = blueprint_engine.cache().latest_at(
                ctx.blueprint_query,
                &visualizer_instruction.override_path,
                [component],
            );

            // If we successfully find a non-empty override, add it to our results.

            // TODO(jleibs): it seems like value could still be null/empty if the override
            // has been cleared. It seems like something is preventing that from happening
            // but I don't fully understand what.
            //
            // This is extra tricky since the promise hasn't been resolved yet so we can't
            // actually look at the data.
            if let Some(value) = component_override_result.get(component) {
                let index = value.index(&ctx.blueprint_query.timeline());

                // NOTE: This can never happen, but I'd rather it happens than an unwrap.
                debug_assert!(index.is_some(), "{value:#?}");
                let index = index.unwrap_or((TimeInt::STATIC, RowId::ZERO));

                overrides.add(component, index, value.clone());
            }
        }
    }
    overrides
}

pub trait DataResultQuery {
    fn latest_at_with_blueprint_resolved_data<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> HybridLatestAtResults<'a>;

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
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
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> HybridLatestAtResults<'a> {
        let query_shadowed_components = false;
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            A::all_component_identifiers(),
            query_shadowed_components,
            visualizer_instruction,
        )
    }

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
        visualizer_instruction: &re_viewer_context::VisualizerInstruction,
    ) -> HybridLatestAtResults<'a> {
        let query_shadowed_components = false;
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            std::iter::once(component),
            query_shadowed_components,
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
