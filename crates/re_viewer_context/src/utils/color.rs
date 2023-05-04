use re_log_types::EntityPath;

// default colors
// Borrowed from `egui::PlotUi`
pub fn auto_color(val: u16) -> re_renderer::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = val as f32 * golden_ratio;
    egui::Color32::from(egui::ecolor::Hsva::new(h, 0.85, 0.5, 1.0))
}

#[derive(Clone, Copy)]
pub enum DefaultColor<'a> {
    OpaqueWhite,
    TransparentBlack,
    EntityPath(&'a EntityPath),
}
