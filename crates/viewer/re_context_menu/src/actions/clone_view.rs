use re_viewer_context::{Item, SystemCommand, SystemCommandSender as _, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

/// Clone a single view
pub(crate) struct CloneViewAction;

impl ContextMenuAction for CloneViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::View(_))
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Clone".to_owned()
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        if let Some(new_view_id) = ctx
            .viewport_blueprint
            .duplicate_view(view_id, ctx.viewer_context)
        {
            ctx.viewer_context
                .command_sender()
                .send_system(SystemCommand::set_selection(Item::View(new_view_id)));
            ctx.viewport_blueprint
                .mark_user_interaction(ctx.viewer_context);
        }
    }
}
