// TODO(jleibs): Turn this into a trait

use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::EntityPath;
use re_query::ComponentWithInstances;
use re_types::components::Color;
use re_viewer_context::{UiVerbosity, ViewerContext};

#[allow(clippy::too_many_arguments)]
pub fn edit_color_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _verbosity: UiVerbosity,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &ComponentWithInstances,
    instance_key: &re_types::components::InstanceKey,
) {
    // TODO(jleibs): Handle missing data still
    if let Ok(current_color) = component.lookup::<Color>(instance_key) {
        let [r, g, b, a] = current_color.to_array();
        let current_color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        let mut edit_color = current_color;

        egui::color_picker::color_edit_button_srgba(
            ui,
            &mut edit_color,
            egui::color_picker::Alpha::Opaque,
        );

        if edit_color != current_color {
            let [r, g, b, a] = edit_color.to_array();
            let new_color = Color::from_unmultiplied_rgba(r, g, b, a);

            ctx.save_blueprint_component(override_path, new_color);
        }
    }
}
