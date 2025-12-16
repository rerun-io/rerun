use re_sdk_types::components::EntityPath;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub(crate) fn edit_or_view_entity_path(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    path: &mut MaybeMutRef<'_, EntityPath>,
) -> egui::Response {
    if let Some(path) = path.as_mut() {
        // A suggestion mechanism similar to the one in `view_space_origin_widget_ui` would be nice.
        let mut string = path.to_string();
        let response = ui.text_edit_singleline(&mut string);
        *path = string.into();

        response
    } else {
        // Assume the current query for information shown in hover cards etc.
        let query = ctx.current_query();

        // Entity paths right now always refer to the current recording.
        // This might change in the future at which point we need more context here.
        let db = ctx.recording();

        let entity_path = path.as_ref().as_str().into();
        re_data_ui::item_ui::entity_path_button(ctx, &query, db, ui, None, &entity_path)
    }
}
