// TODO(jleibs): Turn this into a trait

use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::{DataCell, EntityPath};
use re_query::ComponentWithInstances;
use re_types::{
    components::{Color, Text},
    Loggable,
};
use re_viewer_context::{UiVerbosity, ViewerContext};

#[allow(clippy::too_many_arguments)]
fn edit_color_ui(
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

fn default_color(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
) -> DataCell {
    [Color::from_rgb(255, 255, 255)].into()
}

#[allow(clippy::too_many_arguments)]
fn edit_text_ui(
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
    if let Ok(current_text) = component.lookup::<Text>(instance_key) {
        let current_text = current_text.to_string();
        let mut edit_text = current_text.clone();

        // TODO(jleibs): Clip text false isn't exactly what we want. Need
        // to figure out how to size this properly to fit the space appropriately.
        egui::TextEdit::singleline(&mut edit_text)
            .clip_text(false)
            .show(ui);

        if edit_text != current_text {
            let new_text = Text::from(edit_text);

            ctx.save_blueprint_component(override_path, new_text);
        }
    }
}

fn default_text(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    entity_path: &EntityPath,
) -> DataCell {
    [Text::from(entity_path.to_string())].into()
}

pub fn register_editors(registry: &mut re_viewer_context::ComponentUiRegistry) {
    registry.add_editor(
        re_types::components::Color::name(),
        Box::new(default_color),
        Box::new(edit_color_ui),
    );

    registry.add_editor(
        re_types::components::Text::name(),
        Box::new(default_text),
        Box::new(edit_text_ui),
    );
}
