use re_sdk_types::ViewClassIdentifier;
use re_ui::Icon;
use re_viewer_context::{ContainerId, Item, RecommendedView};
use re_viewport_blueprint::ViewBlueprint;

use crate::{ContextMenuAction, ContextMenuContext};

/// Add a view of the specific class
pub(crate) struct AddViewAction {
    pub icon: &'static Icon,
    pub id: ViewClassIdentifier,
}

impl ContextMenuAction for AddViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::Container(_))
    }

    fn icon(&self) -> Option<&'static re_ui::Icon> {
        Some(self.icon)
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        ctx.viewer_context
            .view_class_registry()
            .get_class_or_log_error(self.id)
            .display_name()
            .to_owned()
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        let view = ViewBlueprint::new(self.id, RecommendedView::root());

        ctx.viewport_blueprint
            .add_views(std::iter::once(view), Some(*container_id), None);
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
    }
}
