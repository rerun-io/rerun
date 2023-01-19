use re_log_types::{field_types::Instance, msg_bundle::Component};
use re_query::ComponentWithInstances;

pub(crate) fn arrow_component_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    component: &ComponentWithInstances,
    preview: crate::ui::Preview,
) {
    let count = component.len();

    let max_elems = match preview {
        crate::ui::Preview::Small | crate::ui::Preview::MaxHeight(_) => 1,
        crate::ui::Preview::Large => 20,
    };

    match component.iter_instance_keys() {
        Ok(mut instance_keys) => {
            if count == 0 {
                ui.weak("(empty)");
            } else if count == 1 {
                if let Some(instance) = instance_keys.next() {
                    arrow_component_elem_ui(ctx, ui, preview, component, &instance);
                } else {
                    ui.label("Error: missing instance key");
                }
            } else if count <= max_elems {
                egui::Grid::new("component").num_columns(2).show(ui, |ui| {
                    for instance in instance_keys {
                        ui.label(format!("{}", instance));
                        arrow_component_elem_ui(ctx, ui, preview, component, &instance);
                        ui.end_row();
                    }
                });
            } else {
                ui.label(format!("{} values", count));
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
    } else {
        ctx.component_ui_registry
            .ui(ctx, ui, preview, component, instance);
    }
}
