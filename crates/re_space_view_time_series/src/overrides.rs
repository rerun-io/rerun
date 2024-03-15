use re_log_types::EntityPath;
use re_types::components::Color;
use re_viewer_context::{DefaultColor, ResolvedAnnotationInfo};

pub fn initial_override_color(entity_path: &EntityPath) -> Color {
    let default_color = DefaultColor::EntityPath(entity_path);

    let annotation_info = ResolvedAnnotationInfo::default();

    let color = annotation_info.color(None, default_color);

    let [r, g, b, a] = color.to_array();

    Color::from_unmultiplied_rgba(r, g, b, a)
}
