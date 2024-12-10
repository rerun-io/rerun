use re_types::SpaceViewClassIdentifier;
use re_ui::Icon;
use re_viewer_context::{ContainerId, Item, RecommendedSpaceView};
use re_viewport_blueprint::SpaceViewBlueprint;

use crate::{ContextMenuAction, ContextMenuContext};

/// Add a view of the specific class
pub(crate) struct AddSpaceViewAction {
    pub icon: &'static Icon,
    pub id: SpaceViewClassIdentifier,
}

impl ContextMenuAction for AddSpaceViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::Container(_))
    }

    fn icon(&self) -> Option<&'static re_ui::Icon> {
        Some(self.icon)
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        ctx.viewer_context
            .space_view_class_registry
            .get_class_or_log_error(self.id)
            .display_name()
            .to_owned()
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        let space_view = SpaceViewBlueprint::new(self.id, RecommendedSpaceView::root());

        ctx.viewport_blueprint.add_space_views(
            std::iter::once(space_view),
            Some(*container_id),
            None,
        );
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
    }
}
