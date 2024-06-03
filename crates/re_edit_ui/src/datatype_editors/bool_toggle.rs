/// Generic editor for a boolean value.
pub fn edit_bool(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut impl std::ops::DerefMut<Target = bool>,
) -> egui::Response {
    edit_bool_impl(ui, value)
}

/// Non monomorphized implementation of [`edit_bool`].
fn edit_bool_impl(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    ui.scope(move |ui| {
        ui.visuals_mut().widgets.hovered.expansion = 0.0;
        ui.visuals_mut().widgets.active.expansion = 0.0;
        ui.add(re_ui::toggle_switch(15.0, value))
    })
    .inner
}
