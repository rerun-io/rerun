//! Context menu action to expand and collapse various trees in the UI.
//!
//! Note: the actual collapse/expand logic is in [`crate::collapse_expand`].

use re_entity_db::InstancePath;
use re_log_types::StoreKind;
use re_viewer_context::{CollapseScope, ContainerId, Item, ItemContext, ViewId};

use crate::collapse_expand::{
    collapse_expand_container, collapse_expand_data_result, collapse_expand_instance_path,
    collapse_expand_view,
};
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
        // TODO(ab): in an ideal world, we'd check the fully expanded/collapsed state of the item to
        // avoid showing a command that wouldn't have an effect but that's lots of added complexity.
        match item {
            Item::AppId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::ComponentPath(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_)
            | Item::TableId(_) => false,

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

        collapse_expand_container(
            ctx.viewer_context,
            ctx.viewport_blueprint,
            container_id,
            scope,
            self.open(),
        );
    }

    fn process_view(&self, ctx: &ContextMenuContext<'_>, view_id: &ViewId) {
        let scope = blueprint_collapse_scope(ctx, &Item::View(*view_id));

        collapse_expand_view(ctx.viewer_context, view_id, scope, self.open());
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        let scope =
            blueprint_collapse_scope(ctx, &Item::DataResult(*view_id, instance_path.clone()));

        collapse_expand_data_result(
            ctx.viewer_context,
            view_id,
            instance_path,
            scope,
            self.open(),
        );
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

        collapse_expand_instance_path(ctx.viewer_context, db, instance_path, scope, self.open());
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
