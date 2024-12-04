use re_types::blueprint::components::ForceLink;
use re_ui::UiExt as _;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn singleline_view_force_link(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ForceLink>,
) -> egui::Response {
    UiLayout::List
        .label(ui, format!("test",))
        .on_hover_ui(|ui| {
            ui.markdown_ui("Toggle link force on/off");
        })
}
