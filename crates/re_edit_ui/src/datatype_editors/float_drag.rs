use egui::NumExt as _;

/// Generic editor for a f32 value from zero to infinity.
pub fn edit_f32_zero_to_inf(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut impl std::ops::DerefMut<Target = f32>,
) -> egui::Response {
    edit_f32_zero_to_inf_impl(ui, value)
}

/// Non monomorphized implementation of [`edit_f32_zero_to_inf`].
fn edit_f32_zero_to_inf_impl(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    let speed = (*value * 0.01).at_least(0.001);
    ui.add(
        egui::DragValue::new(value)
            .clamp_range(0.0..=f32::INFINITY)
            .speed(speed),
    )
}
