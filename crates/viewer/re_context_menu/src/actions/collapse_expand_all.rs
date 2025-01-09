use re_entity_db::InstancePath;
use re_viewer_context::{CollapseScope, ContainerId, Contents, Item, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

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

    /// Do we have a context menu for this item?
    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        // TODO(ab): in an ideal world, we'd check the fully expended/collapsed state of the item to
        // avoid showing a command that wouldn't have an effect but that's lots of added complexity.
        match item {
            Item::AppId(_) | Item::DataSource(_) | Item::StoreId(_) | Item::ComponentPath(_) => {
                false
            }

            Item::View(_) | Item::Container(_) | Item::InstancePath(_) => true,

            //TODO(ab): for DataResult, walk the data result tree instead!
            Item::DataResult(_, instance_path) => ctx
                .viewer_context
                .recording()
                .tree()
                .subtree(&instance_path.entity_path)
                .is_some_and(|subtree| !subtree.is_leaf()),
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        match self {
            Self::CollapseAll => "Collapse all".to_owned(),
            Self::ExpandAll => "Expand all".to_owned(),
        }
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        let collapse_scope = ctx
            .local_date()
            .copied()
            .unwrap_or(CollapseScope::BlueprintTree);

        ctx.viewport_blueprint
            .visit_contents_in_container(container_id, &mut |contents, _| match contents {
                Contents::Container(container_id) => collapse_scope
                    .container(*container_id)
                    .set_open(&ctx.egui_context, self.open()),
                Contents::View(view_id) => self.process_view(ctx, view_id),
            });
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        ctx.local_date()
            .copied()
            .unwrap_or(CollapseScope::BlueprintTree)
            .view(*view_id)
            .set_open(&ctx.egui_context, self.open());

        let query_result = ctx.viewer_context.lookup_query_result(*view_id);
        let result_tree = &query_result.tree;
        if let Some(root_node) = result_tree.root_node() {
            self.process_data_result(
                ctx,
                view_id,
                &InstancePath::entity_all(root_node.data_result.entity_path.clone()),
            );
        }
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        //TODO(ab): here we should in principle walk the DataResult tree instead of the entity tree
        // but the current API isn't super ergonomic.
        let Some(subtree) = get_entity_tree(ctx, instance_path) else {
            return;
        };

        let collapse_scope = ctx
            .local_date()
            .copied()
            .unwrap_or(CollapseScope::BlueprintTree);

        subtree.visit_children_recursively(|entity_path| {
            collapse_scope
                .data_result(*view_id, entity_path.clone())
                .set_open(&ctx.egui_context, self.open());
        });
    }

    fn process_instance_path(&self, ctx: &ContextMenuContext<'_>, instance_path: &InstancePath) {
        let Some(subtree) = get_entity_tree(ctx, instance_path) else {
            return;
        };

        let collapse_scope = ctx
            .local_date()
            .copied()
            .unwrap_or(CollapseScope::StreamsTree);

        subtree.visit_children_recursively(|entity_path| {
            collapse_scope
                .entity(entity_path.clone())
                .set_open(&ctx.egui_context, self.open());
        });
    }
}

impl CollapseExpandAllAction {
    fn open(&self) -> bool {
        match self {
            Self::CollapseAll => false,
            Self::ExpandAll => true,
        }
    }
}

/// Get an [`re_entity_db::EntityTree`] for the given instance path.
///
/// This function guesses which store to search the entity in based on the [`CollapseScope`], which
/// may be overridden as local data by the user code.
fn get_entity_tree<'a>(
    ctx: &'_ ContextMenuContext<'a>,
    instance_path: &InstancePath,
) -> Option<&'a re_entity_db::EntityTree> {
    let collapse_scope = ctx
        .local_date()
        .copied()
        .unwrap_or(CollapseScope::StreamsTree);

    match collapse_scope {
        CollapseScope::StreamsTree | CollapseScope::BlueprintTree => ctx.viewer_context.recording(),
        CollapseScope::BlueprintStreamsTree => ctx.viewer_context.blueprint_db(),
    }
    .tree()
    .subtree(&instance_path.entity_path)
}
