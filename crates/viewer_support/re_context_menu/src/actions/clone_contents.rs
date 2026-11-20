use re_viewer_context::{ContainerId, Item, SystemCommand, SystemCommandSender as _, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

/// Clone a view or an entire container hierarchy.
pub(crate) struct CloneContentsAction;

impl ContextMenuAction for CloneContentsAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::View(_) | Item::Container(_))
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

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        if let Some(new_id) = ctx
            .viewport_blueprint
            .duplicate_container(container_id, ctx.viewer_context)
        {
            ctx.viewer_context
                .command_sender()
                .send_system(SystemCommand::set_selection(Item::Container(new_id)));
        }
    }
}
