use re_data_store::RangeQuery;
use re_types_core::ComponentName;

use re_query::RangeResults;
use re_viewer_context::{ViewQuery, ViewerContext};

// ---

/// Queries for the given `component_names` using range semantics.
///
/// See [`RangeResults`] for more information about how to handle the results.
///
/// This is a cached API -- data will be lazily cached upon access.
pub fn range_with_overrides(
    ctx: &ViewerContext<'_>,
    view_query: &ViewQuery<'_>,
    annotations: &re_viewer_context::Annotations,
    range_query: &RangeQuery,
    data_result: &re_viewer_context::DataResult,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> RangeResults {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    let results = ctx.recording().query_caches().range(
        ctx.recording_store(),
        range_query,
        &data_result.entity_path,
        component_names,
    );

    results
}
