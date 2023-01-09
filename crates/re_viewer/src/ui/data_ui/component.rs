use re_log_types::{external::arrow2::array, field_types::Instance};
use re_query::ComponentWithInstances;

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
    _ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    component: &ComponentWithInstances,
    instance: &Instance,
    _preview: crate::ui::Preview,
) -> egui::Response {
    if let Some(value) = component.lookup(instance) {
        // TODO(jleibs): Dispatch to prettier printers for
        // component types we know about.
        let mut repr = String::new();
        let display = array::get_display(value.as_ref(), "null");
        display(&mut repr, 0).unwrap();
        ui.label(repr)
    } else {
        ui.label("<unset>")
    }
}
