use egui_tiles::ContainerKind;
use re_ui::icons;
use re_viewer_context::{ContainerId, Item};

use crate::{ContextMenuAction, ContextMenuContext};

/// Add a container of a specific type
pub(crate) struct AddContainerAction(pub ContainerKind);

impl ContextMenuAction for AddContainerAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        if let Some(Item::Container(container_id)) = ctx.selection.single_item() {
            if let Some(container) = ctx.viewport_blueprint.container(container_id) {
                let is_linear = matches!(
                    container.container_kind,
                    ContainerKind::Horizontal | ContainerKind::Vertical
                );
                // same-kind linear containers cannot be nested
                !is_linear || container.container_kind != self.0
            } else {
                // unknown container
                false
            }
        } else {
            false
        }
    }

    fn icon(&self) -> Option<&'static re_ui::Icon> {
        match self.0 {
            ContainerKind::Tabs => Some(&icons::CONTAINER_TABS),
            ContainerKind::Horizontal => Some(&icons::CONTAINER_HORIZONTAL),
            ContainerKind::Vertical => Some(&icons::CONTAINER_VERTICAL),
            ContainerKind::Grid => Some(&icons::CONTAINER_GRID),
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
