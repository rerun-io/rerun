use re_viewer_context::{Item, SpaceViewId};

use crate::context_menu::{ContextMenuAction, ContextMenuContext};

/// Clone a single space view
pub(crate) struct CloneSpaceViewAction;

impl ContextMenuAction for CloneSpaceViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::SpaceView(_))
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Clone".to_owned()
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        if let Some(new_space_view_id) = ctx
            .viewport_blueprint
            .duplicate_space_view(space_view_id, ctx.viewer_context)
        {
            ctx.viewer_context
                .selection_state()
                .set_selection(Item::SpaceView(new_space_view_id));
            ctx.viewport_blueprint
                .mark_user_interaction(ctx.viewer_context);
        }
    }
}
