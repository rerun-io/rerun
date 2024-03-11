use re_log_types::{EntityPath, EntityPathFilter};
use re_space_view::SpaceViewBlueprint;
use re_viewer_context::{ContainerId, Item, SpaceViewClassIdentifier};

use crate::context_menu::{ContextMenuAction, ContextMenuContext};

/// Add a space view of the specific class
pub(crate) struct AddSpaceViewAction(pub SpaceViewClassIdentifier);

impl ContextMenuAction for AddSpaceViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::Container(_))
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        ctx.viewer_context
            .space_view_class_registry
            .get_class_or_log_error(&self.0)
            .display_name()
            .to_owned()
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        let space_view =
            SpaceViewBlueprint::new(self.0, &EntityPath::root(), EntityPathFilter::default());

        ctx.viewport_blueprint.add_space_views(
            std::iter::once(space_view),
            ctx.viewer_context,
            Some(*container_id),
            None,
        );
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
    }
}
