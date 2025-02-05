use arrow::{
    array::Array,
    util::display::{ArrayFormatter, FormatOptions},
};
use itertools::Itertools as _;

use re_arrow_util::ArrowArrayDowncastRef as _;

use crate::UiLayout;

pub fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow::array::Array) {
    re_tracing::profile_function!();

    use arrow::array::{LargeListArray, LargeStringArray, ListArray, StringArray};

    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        if array.is_empty() {
            ui.monospace("[]");
            return;
        }

        // Special-treat text.
        // Note: we match on the raw data here, so this works for any component containing text.
        if let Some(entries) = array.downcast_array_ref::<StringArray>() {
            if entries.len() == 1 {
                let string = entries.value(0);
                ui_layout.data_label(ui, string);
                return;
            }
        }
        if let Some(entries) = array.downcast_array_ref::<LargeStringArray>() {
            if entries.len() == 1 {
                let string = entries.value(0);
                ui_layout.data_label(ui, string);
                return;
            }
        }

        // Special-treat batches that are themselves unit-lists (i.e. blobs).
        //
        // What we really want to display in these instances in the underlying array, otherwise we'll
        // bring down the entire viewer trying to render a list whose single entry might itself be
        // an array with millions of values.
        if let Some(entries) = array.downcast_array_ref::<ListArray>() {
            if entries.len() == 1 {
                return arrow_ui(ui, ui_layout, entries.values());
            }
        }
        if let Some(entries) = array.downcast_array_ref::<LargeListArray>() {
            if entries.len() == 1 {
                return arrow_ui(ui, ui_layout, entries.values());
            }
        }

        let instance_count = array.len();

        let options = FormatOptions::default()
            .with_null("null")
            .with_display_error(true);
        if let Ok(formatter) = ArrayFormatter::try_new(array, &options) {
            if instance_count == 1 {
                ui.monospace(formatter.value(0).to_string());
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
        } else {
            // This is unreachable because we use `.with_display_error(true)` above.
        }
    });
}
