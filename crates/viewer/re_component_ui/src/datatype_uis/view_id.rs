use re_data_ui::item_ui;
use re_sdk_types::datatypes::Uuid;
use re_viewer_context::{MaybeMutRef, ViewId};

pub fn view_view_id(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = Uuid>>,
) -> egui::Response {
    // An edit ui could be a drop down with all known views! But that's for another day.
    view_view_id_impl(ctx, ui, value.as_ref())
}

fn view_view_id_impl(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &Uuid,
) -> egui::Response {
    let view = ViewId::from(*value);
    item_ui::blueprint_entity_path_button_to(ctx, ui, &view.as_entity_path(), view.to_string())
}
