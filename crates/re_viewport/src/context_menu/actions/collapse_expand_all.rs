use re_entity_db::InstancePath;
use re_viewer_context::{CollapseScope, ContainerId, Item, SpaceViewId};

use crate::context_menu::{ContextMenuAction, ContextMenuContext};
use crate::Contents;

/// Collapse or expand all items in the selection.
// TODO(ab): the current implementation makes strong assumptions of which CollapseScope to use based
// on the item type. This is brittle and will not scale if/when we add more trees to the UI. When
// that happens, we will have to pass the scope to `context_menu_ui_for_item` and use it here.
pub(crate) enum CollapseExpandAllAction {
    CollapseAll,
    ExpandAll,
}

impl ContextMenuAction for CollapseExpandAllAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        // let's allow this action if at least one item supports it
        ctx.selection
            .iter()
            .any(|(item, _)| self.supports_item(ctx, item))
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        // TODO(ab): in an ideal world, we'd check the fully expended/collapsed state of the item to
        // avoid showing a command that wouldn't have an effect but that's lots of added complexity.
        match item {
            Item::StoreId(_) | Item::ComponentPath(_) => false,
            Item::SpaceView(_) | Item::Container(_) | Item::InstancePath(_) => true,
            //TODO(ab): for DataResult, walk the data result tree instead!
            Item::DataResult(_, instance_path) => ctx
                .viewer_context
                .entity_db
                .tree()
                .subtree(&instance_path.entity_path)
                .is_some_and(|subtree| !subtree.is_leaf()),
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        match self {
            CollapseExpandAllAction::CollapseAll => "Collapse all".to_owned(),
            CollapseExpandAllAction::ExpandAll => "Expand all".to_owned(),
        }
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint
            .visit_contents_in_container(container_id, &mut |contents| match contents {
                Contents::Container(container_id) => CollapseScope::BlueprintTree
                    .container(*container_id)
                    .set_open(&ctx.egui_context, self.open()),
                Contents::SpaceView(space_view_id) => self.process_space_view(ctx, space_view_id),
            });
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        CollapseScope::BlueprintTree
            .space_view(*space_view_id)
            .set_open(&ctx.egui_context, self.open());

        let query_result = ctx.viewer_context.lookup_query_result(*space_view_id);
        let result_tree = &query_result.tree;
        if let Some(root_node) = result_tree.root_node() {
            self.process_data_result(
                ctx,
                space_view_id,
                &InstancePath::entity_splat(root_node.data_result.entity_path.clone()),
            );
        }
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        space_view_id: &SpaceViewId,
        instance_path: &InstancePath,
    ) {
        //TODO(ab): here we should in principle walk the DataResult tree instead of the entity tree
        // but the current API isn't super ergonomic.
        let Some(subtree) = ctx
            .viewer_context
            .entity_db
            .tree()
            .subtree(&instance_path.entity_path)
        else {
            return;
        };

        subtree.visit_children_recursively(&mut |entity_path, _| {
            CollapseScope::BlueprintTree
                .data_result(*space_view_id, entity_path.clone())
                .set_open(&ctx.egui_context, self.open());
        });
    }

    fn process_instance_path(&self, ctx: &ContextMenuContext<'_>, instance_path: &InstancePath) {
        let Some(subtree) = ctx
            .viewer_context
            .entity_db
            .tree()
            .subtree(&instance_path.entity_path)
        else {
            return;
        };

        subtree.visit_children_recursively(&mut |entity_path, _| {
            CollapseScope::StreamsTree
                .entity(entity_path.clone())
                .set_open(&ctx.egui_context, self.open());
        });
    }
}

impl CollapseExpandAllAction {
    fn open(&self) -> bool {
        match self {
            CollapseExpandAllAction::CollapseAll => false,
            CollapseExpandAllAction::ExpandAll => true,
        }
    }
}
