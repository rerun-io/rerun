use nohash_hasher::IntSet;

use re_data_store::{LatestAtQuery, RangeQuery};
use re_query::LatestAtResults;
use re_types_core::ComponentName;
use re_viewer_context::{DataResult, ViewContext, ViewerContext};

use crate::results_ext::{HybridLatestAtResults, HybridRangeResults};

// ---

/// Queries for the given `component_names` using range semantics with override support.
///
/// If the `DataResult` contains a specified override from the blueprint, that values
/// will be used instead of the range query.
///
/// Data should be accessed via the [`crate::RangeResultsExt`] trait which is implemented for
/// [`HybridResults`].
pub fn range_with_overrides(
    ctx: &ViewerContext<'_>,
    _annotations: Option<&re_viewer_context::Annotations>,
    range_query: &RangeQuery,
    data_result: &re_viewer_context::DataResult,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> HybridRangeResults {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    let mut component_set = component_names.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(ctx, data_result, component_set.iter());

    // No need to query for components that have overrides.
    component_set.retain(|component| !overrides.components.contains_key(component));

    let results = ctx.recording().query_caches().range(
        ctx.recording_store(),
        range_query,
        &data_result.entity_path,
        component_set,
    );

    HybridRangeResults { overrides, results }
}

/// Queries for the given `component_names` using latest-at semantics with override support.
///
/// If the `DataResult` contains a specified override from the blueprint, that values
/// will be used instead of the latest-at query.
///
/// Data should be accessed via the [`crate::RangeResultsExt`] trait which is implemented for
/// [`HybridResults`].
pub fn latest_at_with_overrides<'a>(
    ctx: &'a ViewContext<'a>,
    _annotations: Option<&'a re_viewer_context::Annotations>,
    latest_at_query: &LatestAtQuery,
    data_result: &'a re_viewer_context::DataResult,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> HybridLatestAtResults<'a> {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    let mut component_set = component_names.into_iter().collect::<IntSet<_>>();

    let overrides = query_overrides(ctx.viewer_ctx, data_result, component_set.iter());

    // No need to query for components that have overrides.
    component_set.retain(|component| !overrides.components.contains_key(component));

    let results = ctx.viewer_ctx.recording().query_caches().latest_at(
        ctx.viewer_ctx.recording_store(),
        latest_at_query,
        &data_result.entity_path,
        component_set,
    );

    HybridLatestAtResults {
        overrides,
        results,
        ctx,
        query: latest_at_query.clone(),
        data_result,
        resolver: Default::default(),
    }
}

fn query_overrides<'a>(
    ctx: &ViewerContext<'_>,
    data_result: &re_viewer_context::DataResult,
    component_names: impl Iterator<Item = &'a ComponentName>,
) -> LatestAtResults {
    // First see if any components have overrides.
    let mut overrides = LatestAtResults::default();

    if let Some(prop_overrides) = &data_result.property_overrides {
        // TODO(jleibs): partitioning overrides by path
        for component_name in component_names {
            if let Some(override_value) = prop_overrides
                .resolved_component_overrides
                .get(component_name)
            {
                let component_override_result = match override_value.store_kind {
                    re_log_types::StoreKind::Recording => {
                        // TODO(jleibs): This probably is not right, but this code path is not used
                        // currently. This may want to use range_query instead depending on how
                        // component override data-references are resolved.
                        ctx.store_context.blueprint.query_caches().latest_at(
                            ctx.store_context.blueprint.store(),
                            &ctx.current_query(),
                            &override_value.path,
                            [*component_name],
                        )
                    }
                    re_log_types::StoreKind::Blueprint => {
                        ctx.store_context.blueprint.query_caches().latest_at(
                            ctx.store_context.blueprint.store(),
                            ctx.blueprint_query,
                            &override_value.path,
                            [*component_name],
                        )
                    }
                };

                // If we successfully find a non-empty override, add it to our results.

                // TODO(jleibs): it seems like value could still be null/empty if the override
                // has been cleared. It seems like something is preventing that from happening
                // but I don't fully understand what.
                //
                // This is extra tricky since the promise hasn't been resolved yet so we can't
                // actually look at the data.
                if let Some(value) = component_override_result.components.get(component_name) {
                    overrides.add(*component_name, value.clone());
                }
            }
        }
    }
    overrides
}

pub trait DataResultQuery {
    fn latest_at_with_overrides<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
    ) -> HybridLatestAtResults<'a>;

    fn best_fallback_for<'a>(
        &self,
        ctx: &'a ViewContext<'a>,
        component: re_types_core::ComponentName,
    ) -> Option<&'a dyn re_viewer_context::ComponentFallbackProvider>;
}

impl DataResultQuery for DataResult {
    fn latest_at_with_overrides<'a, A: re_types_core::Archetype>(
        &'a self,
        ctx: &'a ViewContext<'a>,
        latest_at_query: &'a LatestAtQuery,
    ) -> HybridLatestAtResults<'a> {
        latest_at_with_overrides(
            ctx,
            None,
            latest_at_query,
            self,
            A::all_components().iter().copied(),
        )
    }

    fn best_fallback_for<'a>(
        &self,
        ctx: &'a ViewContext<'a>,
        component: re_types_core::ComponentName,
    ) -> Option<&'a dyn re_viewer_context::ComponentFallbackProvider> {
        // TODO(jleibs): This should be cached somewhere
        for vis in &self.visualizers {
            let Ok(vis) = ctx.visualizer_collection.get_by_identifier(*vis) else {
                continue;
            };

            if vis.visualizer_query_info().queried.contains(&component) {
                return Some(vis.as_fallback_provider());
            }
        }

        None
    }
}
