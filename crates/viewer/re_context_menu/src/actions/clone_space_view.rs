use re_viewer_context::{Item, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

/// Clone a single view
pub(crate) struct CloneSpaceViewAction;

impl ContextMenuAction for CloneSpaceViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::View(_))
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Clone".to_owned()
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &ViewId) {
        if let Some(new_space_view_id) = ctx
            .viewport_blueprint
            .duplicate_space_view(space_view_id, ctx.viewer_context)
        {
            ctx.viewer_context
                .selection_state()
                .set_selection(Item::View(new_space_view_id));
            ctx.viewport_blueprint
                .mark_user_interaction(ctx.viewer_context);
        }
    }
}
