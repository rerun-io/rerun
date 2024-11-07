use re_types::datatypes::Uuid;
use re_viewer_context::MaybeMutRef;

pub fn view_uuid(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = Uuid>>,
) -> egui::Response {
    ui.label(value.as_ref().to_string())
}
