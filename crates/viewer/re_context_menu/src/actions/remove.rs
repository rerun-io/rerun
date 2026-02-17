use re_entity_db::InstancePath;
use re_viewer_context::{ContainerId, Contents, Item, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

/// Remove a container, view, or data result.
pub(crate) struct RemoveAction;

impl ContextMenuAction for RemoveAction {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::View(_) => true,
            Item::Container(container_id) => ctx.viewport_blueprint.root_container != *container_id,
            Item::DataResult(data_result) => data_result.instance_path.is_all(),
            _ => false,
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Remove".to_owned()
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
        ctx.viewport_blueprint
            .remove_contents(Contents::Container(*container_id));
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
        ctx.viewport_blueprint
            .remove_contents(Contents::View(*view_id));
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        if let Some(view) = ctx.viewport_blueprint.view(view_id) {
            view.contents.remove_subtree_and_matching_rules(
                ctx.viewer_context,
                instance_path.entity_path.clone(),
            );
        }
    }
}
