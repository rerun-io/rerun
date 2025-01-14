use re_entity_db::InstancePath;
use re_log_types::StoreKind;
use re_viewer_context::{CollapseScope, ContainerId, Contents, Item, ItemContext, ViewId};

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
        ctx.viewport_blueprint
            .visit_contents_in_container(container_id, &mut |contents, _| {
                match contents {
                    Contents::Container(container_id) => CollapseScope::BlueprintTree
                        .container(*container_id)
                        .set_open(&ctx.egui_context, self.open()),
                    Contents::View(view_id) => self.process_view(ctx, view_id),
                }

                true
            });
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        CollapseScope::BlueprintTree
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
        let Some(subtree) = ctx
            .viewer_context
            .recording()
            .tree()
            .subtree(&instance_path.entity_path)
        else {
            return;
        };

        subtree.visit_children_recursively(|entity_path| {
            CollapseScope::BlueprintTree
                .data_result(*view_id, entity_path.clone())
                .set_open(&ctx.egui_context, self.open());
        });
    }

    fn process_instance_path(&self, ctx: &ContextMenuContext<'_>, instance_path: &InstancePath) {
        let (db, scope) = match ctx
            .selection
            .context_for_item(&Item::InstancePath(instance_path.clone()))
        {
            Some(&ItemContext::StreamsTree {
                store_kind: StoreKind::Recording,
                filter_session_id: None,
            }) => (ctx.viewer_context.recording(), CollapseScope::StreamsTree),

            Some(&ItemContext::StreamsTree {
                store_kind: StoreKind::Recording,
                filter_session_id: Some(session_id),
            }) => (
                ctx.viewer_context.recording(),
                CollapseScope::StreamsTreeFiltered { session_id },
            ),

            Some(&ItemContext::StreamsTree {
                store_kind: StoreKind::Blueprint,
                filter_session_id: None,
            }) => (
                ctx.viewer_context.blueprint_db(),
                CollapseScope::BlueprintStreamsTree,
            ),

            Some(&ItemContext::StreamsTree {
                store_kind: StoreKind::Blueprint,
                filter_session_id: Some(session_id),
            }) => (
                ctx.viewer_context.blueprint_db(),
                CollapseScope::BlueprintStreamsTreeFiltered { session_id },
            ),

            // default to recording if we don't have more specific information
            Some(&ItemContext::TwoD { .. } | &ItemContext::ThreeD { .. }) | None => {
                (ctx.viewer_context.recording(), CollapseScope::StreamsTree)
            }
        };

        let Some(subtree) = db.tree().subtree(&instance_path.entity_path) else {
            return;
        };

        subtree.visit_children_recursively(|entity_path| {
            scope
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
