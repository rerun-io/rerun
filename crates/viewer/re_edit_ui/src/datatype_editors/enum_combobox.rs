use re_ui::UiExt;
use re_viewer_context::MaybeMutRef;

use crate::response_utils::response_with_changes_of_inner;

pub fn edit_view_enum<EnumT: re_types_core::reflection::Enum + re_types_core::Component>(
    ui: &mut egui::Ui,
    current_value: &mut MaybeMutRef<'_, EnumT>,
) -> egui::Response {
    let id_source = EnumT::name().full_name();
    edit_view_enum_impl(ui, id_source, current_value)
}

fn edit_view_enum_impl<EnumT: re_types_core::reflection::Enum>(
    ui: &mut egui::Ui,
    id_source: &str,
    current_value: &mut MaybeMutRef<'_, EnumT>,
) -> egui::Response {
    if let Some(current_value) = current_value.as_mut() {
        let prev_selected_value = *current_value;

        let mut combobox_response = egui::ComboBox::from_id_source(id_source)
            .selected_text(format!("{current_value}"))
            .height(250.0)
            .show_ui(ui, |ui| {
                ui.set_min_width(60.0);

                let variants = EnumT::variants();
                let mut iter = variants.iter().copied();
                let Some(first) = iter.next() else {
                    return ui.label("<no variants>");
                };

                let mut response = variant_ui(ui, current_value, first);
                for variant in iter {
                    response |= variant_ui(ui, current_value, variant);
                }
                response
            });

        combobox_response.response = combobox_response.response.on_hover_ui(|ui| {
            ui.markdown_ui(
                ui.id().with(prev_selected_value),
                prev_selected_value.docstring_md(),
            );
        });

        response_with_changes_of_inner(combobox_response)
    } else {
        ui.label(current_value.to_string())
    }
}

fn variant_ui<EnumT: re_types_core::reflection::Enum>(
    ui: &mut egui::Ui,
    current_value: &mut EnumT,
    variant: EnumT,
) -> egui::Response {
    ui.selectable_value(current_value, variant, variant.to_string())
        .on_hover_ui(|ui| {
            ui.markdown_ui(ui.id().with(variant), variant.docstring_md());
        })
}
