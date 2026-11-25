#[inline]
pub fn auto_color_egui(val: u16) -> egui::Color32 {
    re_sdk_types::components::Color::auto(val).into()
}

#[inline]
pub fn auto_color_for_entity_path(
    entity_path: &re_entity_db::EntityPath,
) -> re_sdk_types::components::Color {
    re_sdk_types::components::Color::auto_from_hash(entity_path.hash64())
}
