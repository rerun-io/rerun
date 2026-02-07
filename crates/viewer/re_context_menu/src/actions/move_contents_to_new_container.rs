use egui_tiles::ContainerKind;
use re_ui::icons;
use re_viewer_context::Item;

use crate::{ContextMenuAction, ContextMenuContext};

/// Move the selected contents to a newly created container of the given kind
pub(crate) struct MoveContentsToNewContainerAction(pub ContainerKind);

impl ContextMenuAction for MoveContentsToNewContainerAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        if let Some((parent_container, _)) = ctx.clicked_item_enclosing_container_and_position()
            && matches!(
                parent_container.container_kind,
                ContainerKind::Vertical | ContainerKind::Horizontal
            )
            && parent_container.container_kind == self.0
        {
            return false;
        }

        ctx.selection.iter().all(|(item, _)| match item {
            Item::View(_) => true,
            Item::Container(container_id) => ctx.viewport_blueprint.root_container != *container_id,
            _ => false,
        })
    }

    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::View(_) => true,
            Item::Container(container_id) => ctx.viewport_blueprint.root_container != *container_id,
            _ => false,
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

    fn process_selection(&self, ctx: &ContextMenuContext<'_>) {
        let root_container_id = ctx.viewport_blueprint.root_container;
        let (target_container_id, target_position) = ctx
            .clicked_item_enclosing_container_id_and_position()
            .unwrap_or((root_container_id, 0));

        let contents = ctx
            .selection
            .iter()
            .filter_map(|(item, _)| item.try_into().ok())
            .collect();

        ctx.viewport_blueprint.move_contents_to_new_container(
            contents,
            self.0,
            target_container_id,
            target_position,
        );

        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
    }
}
