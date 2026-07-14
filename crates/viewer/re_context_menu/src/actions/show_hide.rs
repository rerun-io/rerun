use re_entity_db::InstancePath;
use re_viewer_context::{ContainerId, Contents, Item, ViewId};

use crate::visibility_actions::{entity_visibility_in_view, set_entity_visibility_in_view};
use crate::{ContextMenuAction, ContextMenuContext};

pub(crate) struct ShowAction;

// TODO(ab): deduplicate these action on the model of CollapseExpandAllAction
impl ContextMenuAction for ShowAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::View(view_id) => !ctx
                .viewport_blueprint
                .is_contents_visible(&Contents::View(*view_id)),
            Item::Container(container_id) => {
                !ctx.viewport_blueprint
                    .is_contents_visible(&Contents::Container(*container_id))
                    && ctx.viewport_blueprint.root_container != *container_id
            }
            Item::DataResult(data_result) => {
                data_result.instance_path.is_all()
                    && entity_visibility_in_view(
                        ctx.viewer_context,
                        data_result.view_id,
                        &data_result.instance_path.entity_path,
                    )
                    .is_some_and(|vis| !vis)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Show all".to_owned()
        } else {
            "Show".to_owned()
        }
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::Container(*container_id),
            true,
        );
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::View(*view_id),
            true,
        );
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_visible(ctx, view_id, instance_path, true);
    }
}

pub(crate) struct HideAction;

impl ContextMenuAction for HideAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::View(view_id) => ctx
                .viewport_blueprint
                .is_contents_visible(&Contents::View(*view_id)),
            Item::Container(container_id) => {
                ctx.viewport_blueprint
                    .is_contents_visible(&Contents::Container(*container_id))
                    && ctx.viewport_blueprint.root_container != *container_id
            }
            Item::DataResult(data_result) => {
                data_result.instance_path.is_all()
                    && entity_visibility_in_view(
                        ctx.viewer_context,
                        data_result.view_id,
                        &data_result.instance_path.entity_path,
                    )
                    .unwrap_or(false)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Hide all".to_owned()
        } else {
            "Hide".to_owned()
        }
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::Container(*container_id),
            false,
        );
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::View(*view_id),
            false,
        );
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_visible(ctx, view_id, instance_path, false);
    }
}

fn set_data_result_visible(
    ctx: &ContextMenuContext<'_>,
    view_id: &ViewId,
    instance_path: &InstancePath,
    visible: bool,
) {
    set_entity_visibility_in_view(
        ctx.viewer_context,
        *view_id,
        &instance_path.entity_path,
        visible,
    );
}
