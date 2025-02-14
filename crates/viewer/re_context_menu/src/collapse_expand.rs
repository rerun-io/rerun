//! Logic that implements the collapse/expand functionality.
//!
//! This is separated from the corresponding context menu action, so it may be reused directly, in
//! particular in tests.

use re_entity_db::{EntityDb, InstancePath};
use re_viewer_context::{
    CollapseScope, ContainerId, Contents, ViewId, ViewerContext, VisitorControlFlow,
};
use re_viewport_blueprint::ViewportBlueprint;

pub fn collapse_expand_container(
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    container_id: &ContainerId,
    scope: CollapseScope,
    expand: bool,
) {
    blueprint.visit_contents_in_container::<()>(container_id, &mut |contents, _| {
        match contents {
            Contents::Container(container_id) => scope
                .container(*container_id)
                .set_open(ctx.egui_ctx(), expand),

            Contents::View(view_id) => collapse_expand_view(ctx, view_id, scope, expand),
        }

        VisitorControlFlow::Continue
    });
}

pub fn collapse_expand_view(
    ctx: &ViewerContext<'_>,
    view_id: &ViewId,
    scope: CollapseScope,
    expand: bool,
) {
    scope.view(*view_id).set_open(ctx.egui_ctx(), expand);

    let query_result = ctx.lookup_query_result(*view_id);
    let result_tree = &query_result.tree;
    if let Some(root_node) = result_tree.root_node() {
        collapse_expand_data_result(
            ctx,
            view_id,
            &InstancePath::entity_all(root_node.data_result.entity_path.clone()),
            scope,
            expand,
        );
    }
}

pub fn collapse_expand_data_result(
    ctx: &ViewerContext<'_>,
    view_id: &ViewId,
    instance_path: &InstancePath,
    scope: CollapseScope,
    expand: bool,
) {
    //TODO(ab): here we should in principle walk the DataResult tree instead of the entity tree
    // but the current API isn't super ergonomic.
    let Some(subtree) = ctx.recording().tree().subtree(&instance_path.entity_path) else {
        return;
    };

    subtree.visit_children_recursively(|entity_path| {
        scope
            .data_result(*view_id, entity_path.clone())
            .set_open(ctx.egui_ctx(), expand);
    });
}

pub fn collapse_expand_instance_path(
    ctx: &ViewerContext<'_>,
    db: &EntityDb,
    instance_path: &InstancePath,
    scope: CollapseScope,
    expand: bool,
) {
    let Some(subtree) = db.tree().subtree(&instance_path.entity_path) else {
        return;
    };

    subtree.visit_children_recursively(|entity_path| {
        scope
            .entity(entity_path.clone())
            .set_open(ctx.egui_ctx(), expand);
    });
}
