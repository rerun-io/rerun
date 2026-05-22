use re_entity_db::EntityPath;

use crate::{DataQueryResult, DataResult, ViewId, ViewerContext};

/// Resolved visibility of `entity_path` in `view_id`, or `None` if the view does not contain it.
pub fn entity_visibility_in_view(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    entity_path: &EntityPath,
) -> Option<bool> {
    ctx.lookup_query_result(view_id)
        .result_for_entity(entity_path)
        .map(DataResult::is_visible)
}

/// Apply `visible` to `entity_path` in `view_id`.
///
/// No-op if the view does not contain the entity.
pub fn set_entity_visibility_in_view(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    entity_path: &EntityPath,
    visible: bool,
) {
    let query_result = ctx.lookup_query_result(view_id);
    if let Some(data_result) = query_result.result_for_entity(entity_path) {
        data_result.save_visible(ctx, &query_result.tree, visible);
    }
}

/// Apply `visible` to `entity_path` in every view whose query result contains it.
///
/// Views whose `ViewContents` does not include `entity_path` (or includes it
/// only as a synthesized tree-prefix ancestor) are skipped: writing an override
/// there would cascade to descendants the user never targeted.
///
/// Views whose resolved visibility already matches `visible` are also skipped:
/// writing a fresh `Some(visible)` would silently sever the entity's
/// parent-to-child visibility inheritance in that view.
pub fn set_entity_visibility_in_all_views(
    ctx: &ViewerContext<'_>,
    entity_path: &EntityPath,
    visible: bool,
) {
    for (_view_id, query_result, data_result) in iter_data_results_for_entity(ctx, entity_path) {
        if data_result.is_visible() != visible {
            data_result.save_visible(ctx, &query_result.tree, visible);
        }
    }
}

/// Returns true iff at least one view other than `excluded_view_id` has a real
/// [`DataResult`] for `entity_path` whose resolved visibility equals
/// `target_visibility`.
pub fn any_other_view_has_entity_visibility(
    ctx: &ViewerContext<'_>,
    excluded_view_id: ViewId,
    entity_path: &EntityPath,
    target_visibility: bool,
) -> bool {
    iter_data_results_for_entity(ctx, entity_path).any(|(view_id, _, data_result)| {
        view_id != excluded_view_id && data_result.is_visible() == target_visibility
    })
}

/// Iterates `(view_id, query_result, data_result)` for every view whose
/// `ViewContents` contains a real (non-prefix-only) [`DataResult`] for
/// `entity_path`.
fn iter_data_results_for_entity<'a>(
    ctx: &'a ViewerContext<'_>,
    entity_path: &'a EntityPath,
) -> impl Iterator<Item = (ViewId, &'a DataQueryResult, &'a DataResult)> + 'a {
    ctx.query_results
        .iter()
        .filter_map(|(&view_id, query_result)| {
            let data_result = query_result.result_for_entity(entity_path)?;
            Some((view_id, query_result, data_result))
        })
}
