use re_data_store::RangeQuery;
use re_types_core::ComponentName;

use re_query::{LatestAtResults, RangeResults};
use re_viewer_context::{external::nohash_hasher::IntSet, ViewerContext};

// ---

/// Wrapper that contains the results of a range query with possible overrides.
pub struct HybridResults {
    pub(crate) overrides: LatestAtResults,
    pub(crate) results: RangeResults,
}

/// Queries for the given `component_names` using range semantics.
///
/// See [`RangeResults`] for more information about how to handle the results.
///
/// This is a cached API -- data will be lazily cached upon access.
pub fn range_with_overrides(
    ctx: &ViewerContext<'_>,
    _annotations: Option<&re_viewer_context::Annotations>,
    range_query: &RangeQuery,
    data_result: &re_viewer_context::DataResult,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> HybridResults {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    let mut component_set = component_names.into_iter().collect::<IntSet<_>>();

    // First see if any components have overrides.
    let mut overrides = LatestAtResults::default();

    if let Some(prop_overrides) = &data_result.property_overrides {
        // TODO(jleibs): partitioning overrides by path
        for component_name in &component_set {
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

    // No need to query for components that have overrides.
    component_set.retain(|component| !overrides.components.contains_key(component));

    let results = ctx.recording().query_caches().range(
        ctx.recording_store(),
        range_query,
        &data_result.entity_path,
        component_set,
    );

    HybridResults { overrides, results }
}
