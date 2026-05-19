use re_sdk_types::components::EntityPath;
use re_viewer_context::{MaybeMutRef, StoreViewContext};

pub(crate) fn edit_or_view_entity_path(
    ctx: &StoreViewContext<'_>,
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
        let entity_path = path.as_ref().as_str().into();
        re_data_ui::item_ui::entity_path_button(ctx, ui, None, &entity_path)
    }
}
