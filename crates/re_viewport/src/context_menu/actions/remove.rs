use re_entity_db::InstancePath;
use re_log_types::EntityPathRule;
use re_viewer_context::{ContainerId, Item, SpaceViewId};

use crate::context_menu::{ContextMenuAction, ContextMenuContext};
use crate::Contents;

/// Remove a container, space view, or data result.
pub(crate) struct RemoveAction;

impl ContextMenuAction for RemoveAction {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::SpaceView(_) => true,
            Item::Container(container_id) => {
                ctx.viewport_blueprint.root_container != Some(*container_id)
            }
            Item::DataResult(_, instance_path) => instance_path.is_splat(),
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

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
        ctx.viewport_blueprint
            .remove_contents(Contents::SpaceView(*space_view_id));
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        space_view_id: &SpaceViewId,
        instance_path: &InstancePath,
    ) {
        if let Some(space_view) = ctx.viewport_blueprint.space_view(space_view_id) {
            space_view.contents.add_entity_exclusion(
                ctx.viewer_context,
                EntityPathRule::including_subtree(instance_path.entity_path.clone()),
            );
        }
    }
}
