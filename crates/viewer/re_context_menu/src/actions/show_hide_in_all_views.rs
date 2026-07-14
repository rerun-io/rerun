use re_entity_db::{EntityPath, InstancePath};
use re_viewer_context::{Item, ViewId};

use crate::visibility_actions::{
    any_view_has_entity_visibility, set_entity_visibility_in_all_views,
};
use crate::{ContextMenuAction, ContextMenuContext};

pub(crate) enum ShowHideInAllViewsAction {
    Show,
    Hide,
}

impl ShowHideInAllViewsAction {
    fn target_visible(&self) -> bool {
        match self {
            Self::Show => true,
            Self::Hide => false,
        }
    }

    /// Enabled iff at least one view contains the entity with the opposite
    /// visibility — i.e. the action would actually change something.
    fn enabled_for_entity(&self, ctx: &ContextMenuContext<'_>, entity_path: &EntityPath) -> bool {
        any_view_has_entity_visibility(ctx.viewer_context, entity_path, !self.target_visible())
    }
}

impl ContextMenuAction for ShowHideInAllViewsAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        // Mirror the rest of the visibility actions: enable if at least one
        // selected item supports the action.
        ctx.selection
            .iter()
            .any(|(item, _)| self.supports_item(ctx, item))
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::DataResult(data_result) => {
                data_result.instance_path.is_all()
                    && self.enabled_for_entity(ctx, &data_result.instance_path.entity_path)
            }
            Item::InstancePath(instance_path) => {
                instance_path.is_all() && self.enabled_for_entity(ctx, &instance_path.entity_path)
            }
            _ => false,
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        match self {
            Self::Show => "Show in all views".to_owned(),
            Self::Hide => "Hide in all views".to_owned(),
        }
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        _view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        set_entity_visibility_in_all_views(
            ctx.viewer_context,
            &instance_path.entity_path,
            self.target_visible(),
        );
    }

    fn process_instance_path(&self, ctx: &ContextMenuContext<'_>, instance_path: &InstancePath) {
        set_entity_visibility_in_all_views(
            ctx.viewer_context,
            &instance_path.entity_path,
            self.target_visible(),
        );
    }
}
