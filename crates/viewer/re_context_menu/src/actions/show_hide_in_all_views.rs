use re_entity_db::InstancePath;
use re_viewer_context::{
    Item, ViewId, any_other_view_has_entity_visibility, set_entity_visibility_in_all_views,
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
}

impl ContextMenuAction for ShowHideInAllViewsAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        // Allow the action if at least one selected item supports it, mirroring
        // the rest of the visibility actions (and `CollapseExpandAllAction`).
        ctx.selection
            .iter()
            .any(|(item, _)| self.supports_item(ctx, item))
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::DataResult(data_result) => {
                data_result.instance_path.is_all()
                    && any_other_view_has_entity_visibility(
                        ctx.viewer_context,
                        data_result.view_id,
                        &data_result.instance_path.entity_path,
                        !self.target_visible(),
                    )
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
}
