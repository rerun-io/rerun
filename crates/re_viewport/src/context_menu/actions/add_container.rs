use re_viewer_context::{ContainerId, Item};

use crate::context_menu::{ContextMenuAction, ContextMenuContext};

/// Add a container of a specific type
pub(crate) struct AddContainerAction(pub egui_tiles::ContainerKind);

impl ContextMenuAction for AddContainerAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        if let Some(Item::Container(container_id)) = ctx.selection.single_item() {
            if let Some(container) = ctx.viewport_blueprint.container(container_id) {
                // same-kind linear containers cannot be nested
                (container.container_kind != egui_tiles::ContainerKind::Vertical
                    && container.container_kind != egui_tiles::ContainerKind::Horizontal)
                    || container.container_kind != self.0
            } else {
                // unknown container
                false
            }
        } else {
            false
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        format!("{:?}", self.0)
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint
            .add_container(self.0, Some(*container_id));
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
    }
}
