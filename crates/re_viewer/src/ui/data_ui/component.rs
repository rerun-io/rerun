use re_log_types::{component_types::Instance, msg_bundle::Component};
use re_query::ComponentWithInstances;

pub(crate) fn arrow_component_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    component: &ComponentWithInstances,
    verbosity: crate::ui::UiVerbosity,
    query: &re_arrow_store::LatestAtQuery,
) {
    let count = component.len();

    let max_elems = match verbosity {
        crate::ui::UiVerbosity::Small | crate::ui::UiVerbosity::MaxHeight(_) => 1,
        crate::ui::UiVerbosity::Large => 20,
    };

    match component.iter_instance_keys() {
        Ok(mut instance_keys) => {
            if count == 0 {
                ui.weak("(empty)");
            } else if count == 1 {
                if let Some(instance) = instance_keys.next() {
                    arrow_component_elem_ui(ctx, ui, verbosity, query, component, &instance);
                } else {
                    ui.label("Error: missing instance key");
                }
            } else if count <= max_elems {
                egui::Grid::new("component").num_columns(2).show(ui, |ui| {
                    for instance in instance_keys {
                        ui.label(format!("{}", instance));
                        arrow_component_elem_ui(ctx, ui, verbosity, query, component, &instance);
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
    verbosity: crate::ui::UiVerbosity,
    query: &re_arrow_store::LatestAtQuery,
    component: &ComponentWithInstances,
    instance_index: &Instance,
) {
    if component.name() == Instance::name() {
        // No reason to do another lookup -- this is the instance itself
        ui.label(instance_index.to_string());
    } else if instance_index.is_splat() {
        arrow_component_ui(ctx, ui, component, verbosity, query);
    } else {
        ctx.component_ui_registry
            .ui(ctx, ui, verbosity, query, component, instance_index);
    }
}
