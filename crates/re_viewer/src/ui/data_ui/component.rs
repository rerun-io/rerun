use re_log_types::{
    external::arrow2::{self, array},
    field_types::Instance,
    msg_bundle::Component,
    AnnotationContext,
};
use re_query::{ComponentWithInstances, QueryError};

use super::DataUi;

pub(crate) fn arrow_component_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    component: &ComponentWithInstances,
    preview: crate::ui::Preview,
) -> egui::Response {
    let count = component.len();

    // TODO(jleibs) be smarter in case of `Specific`
    let max_elems = match preview {
        crate::ui::Preview::Small | crate::ui::Preview::Specific(_) => 1,
        crate::ui::Preview::Medium => 20,
    };

    match component.iter_instance_keys() {
        Ok(mut instance_keys) => {
            if count == 0 {
                ui.label("empty")
            } else if count == 1 {
                if let Some(instance) = instance_keys.next() {
                    arrow_component_elem_ui(ctx, ui, component, &instance, preview)
                } else {
                    ui.label("Error: missing instance key")
                }
            } else if count <= max_elems {
                egui::Grid::new("component")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for instance in instance_keys {
                            ui.label(format!("{}", instance));
                            arrow_component_elem_ui(ctx, ui, component, &instance, preview);
                            ui.end_row();
                        }
                    })
                    .response
            } else {
                ui.label(format!("{} values", count))
            }
        }
        Err(err) => ui.label(format!("Error: {}", err)),
    }
}

pub(crate) fn arrow_component_elem_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    component: &ComponentWithInstances,
    instance: &Instance,
    preview: crate::ui::Preview,
) -> egui::Response {
    // TODO(jleibs): More generic dispatch for arbitrary components
    if component.name() == AnnotationContext::name() {
        match component.lookup::<AnnotationContext>(instance) {
            Ok(annotations) => annotations.data_ui(ctx, ui, preview),
            Err(QueryError::ComponentNotFound) => ui.label("<unset>"),
            Err(err) => ui.label(format!("Error: {}", err)),
        }
    } else if component.name() == Instance::name() {
        // No reason to do another lookup -- this is the instance itself
        ui.label(format!("{}", instance))
    } else if let Some(value) = component.lookup_arrow(instance) {
        let bytes = arrow2::compute::aggregate::estimated_bytes_size(value.as_ref());
        // For small items, print them
        if bytes < 256 {
            let mut repr = String::new();
            let display = array::get_display(value.as_ref(), "null");
            display(&mut repr, 0).unwrap();
            ui.label(repr)
        } else {
            ui.label(format!("{} bytes", bytes))
        }
    } else {
        ui.label("<unset>")
    }
}
