use re_log_types::EntityPath;
use re_viewer_context::ViewerContext;
use re_viewport_blueprint::{ViewBlueprint, default_created_views};

/// Display the space origin of a view.
pub fn view_space_origin_widget_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    view: &ViewBlueprint,
) {
    // All suggestions for this class of views.
    // TODO(#4895): we should have/use a much simpler heuristic API to get a list of compatible entity sub-tree
    let suggestions = || -> Vec<EntityPath> {
        default_created_views(ctx)
            .into_iter()
            .filter(|this_view| this_view.class_identifier() == view.class_identifier())
            .map(|this_view| this_view.space_origin)
            .collect()
    };

    if let Some(new_space_origin) = re_data_ui::entity_path_edit_ui(
        ui,
        view.id.hash(),
        &view.space_origin,
        &suggestions,
        &|space_origin| view.set_origin(ctx, space_origin),
    ) {
        view.set_origin(ctx, &new_space_origin);
    }
}
