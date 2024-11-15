use re_entity_db::InstancePath;
use re_viewer_context::{ContainerId, Contents, Item, SpaceViewId};

use crate::{ContextMenuAction, ContextMenuContext};

pub(crate) struct ShowAction;

// TODO(ab): deduplicate these action on the model of CollapseExpandAllAction
impl ContextMenuAction for ShowAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::SpaceView(space_view_id) => !ctx
                .viewport_blueprint
                .is_contents_visible(&Contents::SpaceView(*space_view_id)),
            Item::Container(container_id) => {
                !ctx.viewport_blueprint
                    .is_contents_visible(&Contents::Container(*container_id))
                    && ctx.viewport_blueprint.root_container != *container_id
            }
            Item::DataResult(space_view_id, instance_path) => {
                data_result_visible(ctx, space_view_id, instance_path).is_some_and(|vis| !vis)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Show All".to_owned()
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

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::SpaceView(*space_view_id),
            true,
        );
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        space_view_id: &SpaceViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_visible(ctx, space_view_id, instance_path, true);
    }
}

pub(crate) struct HideAction;

impl ContextMenuAction for HideAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::SpaceView(space_view_id) => ctx
                .viewport_blueprint
                .is_contents_visible(&Contents::SpaceView(*space_view_id)),
            Item::Container(container_id) => {
                ctx.viewport_blueprint
                    .is_contents_visible(&Contents::Container(*container_id))
                    && ctx.viewport_blueprint.root_container != *container_id
            }
            Item::DataResult(space_view_id, instance_path) => {
                data_result_visible(ctx, space_view_id, instance_path).unwrap_or(false)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Hide All".to_owned()
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

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::SpaceView(*space_view_id),
            false,
        );
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        space_view_id: &SpaceViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_visible(ctx, space_view_id, instance_path, false);
    }
}

fn data_result_visible(
    ctx: &ContextMenuContext<'_>,
    space_view_id: &SpaceViewId,
    instance_path: &InstancePath,
) -> Option<bool> {
    instance_path
        .is_all()
        .then(|| {
            let query_result = ctx.viewer_context.lookup_query_result(*space_view_id);
            query_result
                .tree
                .lookup_result_by_path(&instance_path.entity_path)
                .map(|data_result| data_result.is_visible(ctx.viewer_context))
        })
        .flatten()
}

fn set_data_result_visible(
    ctx: &ContextMenuContext<'_>,
    space_view_id: &SpaceViewId,
    instance_path: &InstancePath,
    visible: bool,
) {
    if let Some(query_result) = ctx.viewer_context.query_results.get(space_view_id) {
        if let Some(data_result) = query_result
            .tree
            .lookup_result_by_path(&instance_path.entity_path)
        {
            data_result.save_recursive_override_or_clear_if_redundant(
                ctx.viewer_context,
                &query_result.tree,
                &re_types::blueprint::components::Visible::from(visible),
            );
        }
    } else {
        re_log::error!("No query available for space view {:?}", space_view_id);
    }
}
