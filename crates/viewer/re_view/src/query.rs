use arrow::array::ArrayRef;
use nohash_hasher::IntSet;

use crate::{
    results_ext::{HybridLatestAtResults, HybridRangeResults},
    HybridResults,
};
use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::{TimeInt, Timeline};
use re_query::LatestAtResults;
use re_types_core::{Archetype, ComponentName};
use re_viewer_context::{
    DataResult, QueryContext, QueryRange, ViewContext, ViewQuery, ViewerContext,
};

// ---

/// Queries for the given `component_names` using range semantics with blueprint support.
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
    component_names: impl IntoIterator<Item = ComponentName>,
) -> HybridRangeResults<'a> {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    let mut component_name_set = component_names.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(ctx.viewer_ctx, data_result, component_name_set.iter());

    // No need to query for components that have overrides.
    component_name_set.retain(|component| !overrides.components.contains_key(component));

    let results = ctx.recording_engine().cache().range(
        range_query,
        &data_result.entity_path,
        component_name_set.iter(),
    );

    HybridRangeResults {
        overrides,
        results,
        defaults: &ctx.query_result.component_defaults,
    }
}

/// Queries for the given `component_names` using latest-at semantics with blueprint support.
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
    component_names: impl IntoIterator<Item = ComponentName>,
    query_shadowed_components: bool,
) -> HybridLatestAtResults<'a> {
    // This is called very frequently, don't put a profile scope here.

    let mut component_set = component_names.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(ctx.viewer_ctx, data_result, component_set.iter());

    // No need to query for components that have overrides unless opted in!
    if !query_shadowed_components {
        component_set.retain(|component| !overrides.components.contains_key(component));
    }

    let results = ctx.viewer_ctx.recording_engine().cache().latest_at(
        latest_at_query,
        &data_result.entity_path,
        component_set.iter().copied(),
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
    timeline: &Timeline,
    timeline_cursor: TimeInt,
    query_range: &QueryRange,
    component_names: impl IntoIterator<Item = ComponentName>,
    data_result: &'a re_viewer_context::DataResult,
) -> HybridResults<'a> {
    match query_range {
        QueryRange::TimeRange(time_range) => {
            let range_query = RangeQuery::new(
                *timeline,
                re_log_types::ResolvedTimeRange::from_relative_time_range(
                    time_range,
                    timeline_cursor,
                ),
            );
            let results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &range_query,
                data_result,
                component_names,
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
                component_names,
                query_shadowed_defaults,
            );
            (latest_query, results).into()
        }
    }
}

fn query_overrides<'a>(
    ctx: &ViewerContext<'_>,
    data_result: &re_viewer_context::DataResult,
    component_names: impl Iterator<Item = &'a ComponentName>,
) -> LatestAtResults {
    // First see if any components have overrides.
    let mut overrides = LatestAtResults::empty("<overrides>".into(), ctx.current_query());

    let blueprint_engine = &ctx.store_context.blueprint.storage_engine();

    // TODO(jleibs): partitioning overrides by path
    for component_name in component_names {
        if let Some(override_value) = data_result
            .property_overrides
            .resolved_component_overrides
            .get(component_name)
        {
            let current_query = match override_value.store_kind {
                re_log_types::StoreKind::Recording => ctx.current_query(),
                re_log_types::StoreKind::Blueprint => ctx.blueprint_query.clone(),
            };

            #[allow(clippy::match_same_arms)] // see @jleibs comment below
            let component_override_result = match override_value.store_kind {
                re_log_types::StoreKind::Recording => {
                    // TODO(jleibs): This probably is not right, but this code path is not used
                    // currently. This may want to use range_query instead depending on how
                    // component override data-references are resolved.
                    blueprint_engine.cache().latest_at(
                        &current_query,
                        &override_value.path,
                        [*component_name],
                    )
                }
                re_log_types::StoreKind::Blueprint => blueprint_engine.cache().latest_at(
                    &current_query,
                    &override_value.path,
                    [*component_name],
                ),
            };

            // If we successfully find a non-empty override, add it to our results.

            // TODO(jleibs): it seems like value could still be null/empty if the override
            // has been cleared. It seems like something is preventing that from happening
            // but I don't fully understand what.
            //
            // This is extra tricky since the promise hasn't been resolved yet so we can't
            // actually look at the data.
            if let Some(value) = component_override_result.components.get(component_name) {
                let index = value.index(&current_query.timeline());

                // NOTE: This can never happen, but I'd rather it happens than an unwrap.
                debug_assert!(index.is_some(), "{value:#?}");
                let index = index.unwrap_or((TimeInt::STATIC, RowId::ZERO));

                overrides.add(*component_name, index, value.clone());
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

    fn query_archetype_with_history<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
    ) -> HybridResults<'a>;

    fn best_fallback_for<'a>(
        &self,
        query_ctx: &'a QueryContext<'a>,
        visualizer_collection: &'a re_viewer_context::VisualizerCollection,
        component: re_types_core::ComponentName,
    ) -> ArrayRef;
}

impl DataResultQuery for DataResult {
    fn latest_at_with_blueprint_resolved_data<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
    ) -> HybridLatestAtResults<'a> {
        let query_shadowed_defaults = false;
        latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            latest_at_query,
            self,
            A::all_components().iter().map(|descr| descr.component_name),
            query_shadowed_defaults,
        )
    }

    fn query_archetype_with_history<'a, A: Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        view_query: &ViewQuery<'_>,
    ) -> HybridResults<'a> {
        query_archetype_with_history(
            ctx,
            &view_query.timeline,
            view_query.latest_at,
            self.query_range(),
            A::all_components().iter().map(|descr| descr.component_name),
            self,
        )
    }

    fn best_fallback_for<'a>(
        &self,
        query_ctx: &'a QueryContext<'a>,
        visualizer_collection: &'a re_viewer_context::VisualizerCollection,
        component: re_types_core::ComponentName,
    ) -> ArrayRef {
        // TODO(jleibs): This should be cached somewhere
        for vis in &self.visualizers {
            let Ok(vis) = visualizer_collection.get_by_identifier(*vis) else {
                continue;
            };

            if vis.visualizer_query_info().queried.contains(&component) {
                return vis.fallback_provider().fallback_for(query_ctx, component);
            }
        }

        query_ctx.viewer_ctx.placeholder_for(component)
    }
}
