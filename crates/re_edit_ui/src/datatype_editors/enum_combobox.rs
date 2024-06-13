use crate::response_utils::response_with_changes_of_inner;

pub fn edit_enum<Value: PartialEq + std::fmt::Display + Copy>(
    ui: &mut egui::Ui,
    id_source: &str,
    value: &mut Value,
    variants: &[Value],
) -> egui::Response {
    if ui.is_enabled() {
        response_with_changes_of_inner(
            egui::ComboBox::from_id_source(id_source)
                .selected_text(format!("{value}"))
                .show_ui(ui, |ui| {
                    let mut iter = variants.iter().copied();
                    let Some(first) = iter.next() else {
                        return ui.label("<no variants>");
                    };

                    let mut response = ui.selectable_value(value, first, format!("{first}"));
                    for variant in iter {
                        response |= ui.selectable_value(value, variant, format!("{variant}"));
                    }
                    response
                }),
        )
    } else {
        // Don't show the combo box drop down if it's disabled.
        ui.selectable_label(false, format!("{value}"))
    }
}
