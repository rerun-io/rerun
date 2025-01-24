use arrow::util::display::{ArrayFormatter, FormatOptions};
use itertools::Itertools as _;

use re_arrow_util::ArrowArrayDowncastRef as _;

use crate::UiLayout;

pub fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow::array::Array) {
    use arrow::array::{LargeStringArray, StringArray};

    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        if array.is_empty() {
            ui.monospace("[]");
            return;
        }

        // Special-treat text.
        // Note: we match on the raw data here, so this works for any component containing text.
        if let Some(utf8) = array.downcast_array_ref::<StringArray>() {
            if utf8.values().len() == 1 {
                let string = utf8.value(0);
                ui_layout.data_label(ui, string);
                return;
            }
        }
        if let Some(utf8) = array.downcast_array_ref::<LargeStringArray>() {
            if utf8.values().len() == 1 {
                let string = utf8.value(0);
                ui_layout.data_label(ui, string);
                return;
            }
        }

        let num_bytes = array.get_buffer_memory_size();
        if num_bytes < 3_000 {
            let instance_count = array.len();

            let options = FormatOptions::default();
            if let Ok(formatter) = ArrayFormatter::try_new(array, &options) {
                if instance_count == 1 {
                    ui.monospace(formatter.value(0).to_string());
                    return;
                } else {
                    let response = ui_layout.label(ui, format!("{instance_count} items"));

                    if instance_count < 100 {
                        response.on_hover_ui(|ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            ui.monospace(format!(
                                "[{}]",
                                (0..instance_count)
                                    .map(|index| formatter.value(index).to_string())
                                    .join(", ")
                            ));
                        });
                    }
                }
            }
            return;
        }

        // Fallback:
        let bytes = re_format::format_bytes(num_bytes as _);

        // TODO(emilk): pretty-print data type
        let data_type_formatted = format!("{:?}", array.data_type());

        if data_type_formatted.len() < 20 {
            // e.g. "4.2 KiB of Float32"
            ui_layout.label(ui, format!("{bytes} of {data_type_formatted}"));
        } else {
            // Huge datatype, probably a union horror show
            ui_layout.label(ui, format!("{bytes} of data"));
        }
    });
}
