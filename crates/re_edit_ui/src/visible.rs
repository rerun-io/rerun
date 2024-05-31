use re_types::blueprint::components::Visible;
use re_viewer_context::ViewerContext;

pub fn edit_visible(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Visible,
) -> egui::Response {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.hovered.expansion = 0.0;
        ui.visuals_mut().widgets.active.expansion = 0.0;
        ui.add(re_ui::toggle_switch(15.0, &mut value.0))
    })
    .inner
}
