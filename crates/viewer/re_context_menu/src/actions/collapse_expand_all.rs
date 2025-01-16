use re_entity_db::InstancePath;
use re_log_types::StoreKind;
use re_viewer_context::{CollapseScope, ContainerId, Contents, Item, ItemContext, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

/// Collapse or expand all items in the selection.
///
/// Note: this makes _heavy_ use of [`ItemContext`] to determine the correct scope to
/// collapse/expand.
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
        let scope = blueprint_collapse_scope(ctx, &Item::Container(*container_id));

        ctx.viewport_blueprint
            .visit_contents_in_container(container_id, &mut |contents, _| {
                match contents {
                    Contents::Container(container_id) => scope
                        .container(*container_id)
                        .set_open(&ctx.egui_context, self.open()),

                    // IMPORTANT: don't call process_view() here, or the scope information would be lost
                    Contents::View(view_id) => self.process_view_impl(ctx, view_id, scope),
                }

                true
            });
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        let scope = blueprint_collapse_scope(ctx, &Item::View(*view_id));

        self.process_view_impl(ctx, view_id, scope);
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        let scope =
            blueprint_collapse_scope(ctx, &Item::DataResult(*view_id, instance_path.clone()));

        self.process_data_result_impl(ctx, view_id, instance_path, scope);
    }

    fn process_instance_path(&self, ctx: &ContextMenuContext<'_>, instance_path: &InstancePath) {
        let (db, scope) = match ctx
            .selection
            .context_for_item(&Item::InstancePath(instance_path.clone()))
        {
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

            // default to recording if the item context explicitly points to it or if we don't have
            // any relevant context
            Some(
                &ItemContext::StreamsTree {
                    store_kind: StoreKind::Recording,
                    filter_session_id: None,
                }
                | &ItemContext::TwoD { .. }
                | &ItemContext::ThreeD { .. }
                | &ItemContext::BlueprintTree { .. },
            )
            | None => (ctx.viewer_context.recording(), CollapseScope::StreamsTree),
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

    fn process_view_impl(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        scope: CollapseScope,
    ) {
        scope
            .view(*view_id)
            .set_open(&ctx.egui_context, self.open());

        let query_result = ctx.viewer_context.lookup_query_result(*view_id);
        let result_tree = &query_result.tree;
        if let Some(root_node) = result_tree.root_node() {
            self.process_data_result_impl(
                ctx,
                view_id,
                &InstancePath::entity_all(root_node.data_result.entity_path.clone()),
                scope,
            );
        }
    }

    fn process_data_result_impl(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
        scope: CollapseScope,
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
            scope
                .data_result(*view_id, entity_path.clone())
                .set_open(&ctx.egui_context, self.open());
        });
    }
}

/// Determine the [`CollapseScope`] to use for items in the blueprint tree.
fn blueprint_collapse_scope(ctx: &ContextMenuContext<'_>, item: &Item) -> CollapseScope {
    match ctx.selection.context_for_item(item) {
        Some(&ItemContext::BlueprintTree {
            filter_session_id: Some(session_id),
        }) => CollapseScope::BlueprintTreeFiltered { session_id },

        None
        | Some(
            &ItemContext::BlueprintTree {
                filter_session_id: None,
            }
            | &ItemContext::StreamsTree { .. }
            | &ItemContext::TwoD { .. }
            | &ItemContext::ThreeD { .. },
        ) => CollapseScope::BlueprintTree,
    }
}
