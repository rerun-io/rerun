// default colors
// Borrowed from `egui::PlotUi`
#[inline]
pub fn auto_color_egui(val: u16) -> egui::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = val as f32 * golden_ratio;
    egui::Color32::from(egui::ecolor::Hsva::new(h, 0.85, 0.5, 1.0))
}

#[inline]
pub fn auto_color_for_entity_path(
    entity_path: &re_entity_db::EntityPath,
) -> re_sdk_types::components::Color {
    auto_color_egui((entity_path.hash64() % u16::MAX as u64) as u16).into()
}
