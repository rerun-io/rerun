use re_entity_db::InstancePath;
use re_viewer_context::{Item, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

/// Returns the resolved interactivity of `entity_path` in `view_id`, or `None` if
/// the view does not contain it.
fn entity_interactivity_in_view(
    ctx: &re_viewer_context::ViewerContext<'_>,
    view_id: ViewId,
    entity_path: &re_entity_db::EntityPath,
) -> Option<bool> {
    ctx.lookup_query_result(view_id)
        .result_for_entity(entity_path)
        .map(|dr| dr.is_interactive())
}

fn set_entity_interactivity_in_view(
    ctx: &re_viewer_context::ViewerContext<'_>,
    view_id: ViewId,
    entity_path: &re_entity_db::EntityPath,
    interactive: bool,
) {
    let query_result = ctx.lookup_query_result(view_id);
    if let Some(data_result) = query_result.result_for_entity(entity_path) {
        data_result.save_interactive(ctx, &query_result.tree, interactive);
    }
}

/// Make the selected items non-interactive
pub(crate) struct LockAction;

impl ContextMenuAction for LockAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| {
            if let Item::DataResult(dr) = item {
                dr.instance_path.is_all()
                    && entity_interactivity_in_view(
                        ctx.viewer_context,
                        dr.view_id,
                        &dr.instance_path.entity_path,
                    )
                    .unwrap_or(false)
            } else {
                false
            }
        })
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Lock".to_owned()
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        set_entity_interactivity_in_view(
            ctx.viewer_context,
            *view_id,
            &instance_path.entity_path,
            false,
        );
    }
}

/// Make the selected items interactive again
pub(crate) struct UnlockAction;

impl ContextMenuAction for UnlockAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| {
            if let Item::DataResult(dr) = item {
                dr.instance_path.is_all()
                    && entity_interactivity_in_view(
                        ctx.viewer_context,
                        dr.view_id,
                        &dr.instance_path.entity_path,
                    )
                    .is_some_and(|interactive| !interactive)
            } else {
                false
            }
        })
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Unlock".to_owned()
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        set_entity_interactivity_in_view(
            ctx.viewer_context,
            *view_id,
            &instance_path.entity_path,
            true,
        );
    }
}
