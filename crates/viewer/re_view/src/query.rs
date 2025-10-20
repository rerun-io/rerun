use nohash_hasher::IntSet;

use crate::{
    HybridResults,
    results_ext::{HybridLatestAtResults, HybridRangeResults},
};

use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::{TimeInt, TimelineName};
use re_query::LatestAtResults;
use re_types_core::{Archetype, ComponentIdentifier};
use re_viewer_context::{DataResult, QueryRange, ViewContext, ViewQuery, ViewerContext};

// ---

/// Queries for the given `component_types` using range semantics with blueprint support.
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
) -> HybridRangeResults<'a> {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    let mut components = components.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(ctx.viewer_ctx, data_result, components.iter().copied());

    // No need to query for components that have overrides.
    components.retain(|component| overrides.get(*component).is_none());

    let results =
        ctx.recording_engine()
            .cache()
            .range(range_query, &data_result.entity_path, components);

    HybridRangeResults {
        overrides,
        results,
        defaults: &ctx.query_result.component_defaults,
    }
}

/// Queries for the given `component_types` using latest-at semantics with blueprint support.
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
// TODO(#10460): Should take identifiers instead of descriptors.
pub fn latest_at_with_blueprint_resolved_data<'a>(
    ctx: &'a ViewContext<'a>,
    _annotations: Option<&'a re_viewer_context::Annotations>,
    latest_at_query: &LatestAtQuery,
    data_result: &'a re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    query_shadowed_components: bool,
) -> HybridLatestAtResults<'a> {
    // This is called very frequently, don't put a profile scope here.

    let mut components = components.into_iter().collect::<IntSet<_>>();
    let overrides = query_overrides(ctx.viewer_ctx, data_result, components.iter().copied());

    // No need to query for components that have overrides unless opted in!
    if !query_shadowed_components {
        components.retain(|component| overrides.get(*component).is_none());
    }

    let results = ctx.viewer_ctx.recording_engine().cache().latest_at(
        latest_at_query,
        &data_result.entity_path,
        components,
    );

    HybridLatestAtResults {
        overrides,
        results,
        defaults: &ctx.query_result.component_defaults,
        ctx,
        query: latest_at_query.clone(),
        data_result,
    }
}

pub fn query_archetype_with_history<'a>(
    ctx: &'a ViewContext<'a>,
    timeline: &TimelineName,
    timeline_cursor: TimeInt,
    query_range: &QueryRange,
    components: impl IntoIterator<Item = ComponentIdentifier>,
    data_result: &'a re_viewer_context::DataResult,
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
            );
            (latest_query, results).into()
        }
    }
}

fn query_overrides(
    ctx: &ViewerContext<'_>,
    data_result: &re_viewer_context::DataResult,
    components: impl IntoIterator<Item = ComponentIdentifier>,
) -> LatestAtResults {
    // First see if any components have overrides.
    let mut overrides = LatestAtResults::empty("<overrides>".into(), ctx.current_query());

    let blueprint_engine = &ctx.store_context.blueprint.storage_engine();

    // TODO(jleibs): partitioning overrides by path
    for component in components {
        if let Some(override_value) = data_result
            .property_overrides
            .component_overrides
            .get(&component)
        {
            let current_query = match override_value.store_kind {
                re_log_types::StoreKind::Recording => ctx.current_query(),
                re_log_types::StoreKind::Blueprint => ctx.blueprint_query.clone(),
            };

            let component_override_result = match override_value.store_kind {
                re_log_types::StoreKind::Recording => {
                    // TODO(jleibs): This probably is not right, but this code path is not used
                    // currently. This may want to use range_query instead depending on how
                    // component override data-references are resolved.
                    blueprint_engine.cache().latest_at(
                        &current_query,
                        &override_value.path,
                        [component],
                    )
                }
                re_log_types::StoreKind::Blueprint => blueprint_engine.cache().latest_at(
                    &current_query,
                    &override_value.path,
                    [component],
                ),
            };

            // If we successfully find a non-empty override, add it to our results.

            // TODO(jleibs): it seems like value could still be null/empty if the override
            // has been cleared. It seems like something is preventing that from happening
            // but I don't fully understand what.
            //
            // This is extra tricky since the promise hasn't been resolved yet so we can't
            // actually look at the data.
            if let Some(value) = component_override_result.get(component) {
                let index = value.index(&current_query.timeline());

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
    ) -> HybridLatestAtResults<'a>;

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
    ) -> HybridLatestAtResults<'a>;

    /// Queries for the given components, taking into account:
    /// * visible history if enabled
    /// * blueprint overrides & defaults
    fn query_components_with_history<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        component_descriptors: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> HybridResults<'a>;

    /// Queries for all components of an archetype, taking into account:
    /// * visible history if enabled
    /// * blueprint overrides & defaults
    fn query_archetype_with_history<'a, A: Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
    ) -> HybridResults<'a> {
        self.query_components_with_history(ctx, view_query, A::all_component_identifiers())
    }
}

impl DataResultQuery for DataResult {
    fn latest_at_with_blueprint_resolved_data<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
    ) -> HybridLatestAtResults<'a> {
        let query_shadowed_components = false;
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            A::all_component_identifiers(),
            query_shadowed_components,
        )
    }

    fn latest_at_with_blueprint_resolved_data_for_component<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
        component: ComponentIdentifier,
    ) -> HybridLatestAtResults<'a> {
        let query_shadowed_components = false;
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            std::iter::once(component),
            query_shadowed_components,
        )
    }

    fn query_components_with_history<'a>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
        components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> HybridResults<'a> {
        query_archetype_with_history(
            ctx,
            &view_query.timeline,
            view_query.latest_at,
            self.query_range(),
            components,
            self,
        )
    }
}
