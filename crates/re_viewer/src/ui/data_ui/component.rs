use re_log_types::{field_types::Instance, msg_bundle::Component};
use re_query::ComponentWithInstances;

use super::DataUi;

pub(crate) fn arrow_component_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    component: &ComponentWithInstances,
    preview: crate::ui::Preview,
) {
    let count = component.len();

    // TODO(jleibs) be smarter in case of `MaxHeight`
    let max_elems = match preview {
        crate::ui::Preview::Small | crate::ui::Preview::MaxHeight(_) => 1,
        crate::ui::Preview::Large => 20,
    };

    match component.iter_instance_keys() {
        Ok(mut instance_keys) => {
            if count == 0 {
                ui.label("empty");
            } else if count == 1 {
                if let Some(instance) = instance_keys.next() {
                    arrow_component_elem_ui(ctx, ui, preview, component, &instance);
                } else {
                    ui.label("Error: missing instance key");
                }
            } else if count <= max_elems {
                egui::Grid::new("component").num_columns(2).show(ui, |ui| {
                    for instance in instance_keys {
                        // We ignore unset/null components
                        if let Some(_) = component.lookup_arrow(&instance) {
                            ui.label(format!("{}", instance));
                            arrow_component_elem_ui(ctx, ui, preview, component, &instance);
                            ui.end_row();
                        }
                    }
                });
            } else {
                ui.label(format!("{} values", count)); // TODO: write component name
            }
        }
        Err(err) => {
            ui.label(format!("Error: {}", err));
        }
    }
}

pub(crate) fn arrow_component_elem_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    preview: crate::ui::Preview,
    component: &ComponentWithInstances,
    instance: &Instance,
) {
    if component.name() == Instance::name() {
        // No reason to do another lookup -- this is the instance itself
        ui.label(format!("{}", instance));
    } else if component.name() == re_log_types::field_types::ColorRGBA::name() {
        // Explicit test  - TODO: remove
        match component.lookup::<re_log_types::field_types::ColorRGBA>(instance) {
            Ok(component) => component.data_ui(ctx, ui, preview),
            Err(re_query::QueryError::ComponentNotFound) => {
                ui.weak("(color not found)");
            }
            Err(err) => {
                ui.label(format!("color error: {}", err));
            }
        }
    } else {
        ctx.component_ui_registry
            .ui(ctx, ui, preview, component, instance);
    }
}
