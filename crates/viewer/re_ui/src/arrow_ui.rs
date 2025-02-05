use arrow::{
    array::Array,
    datatypes::DataType,
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
            ui_layout.data_label(ui, "[]");
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
                ui_layout.data_label(ui, formatter.value(0).to_string());
            } else if instance_count < 10
                && (array.data_type().is_primitive()
                    || matches!(array.data_type(), DataType::Utf8 | DataType::LargeUtf8))
            {
                // A short list of floats, strings, etc. Show it to the user.
                let list_string = format!(
                    "[{}]",
                    (0..instance_count)
                        .map(|index| formatter.value(index).to_string())
                        .join(", ")
                );
                ui_layout.data_label(ui, list_string);
            } else {
                let instance_count_str = re_format::format_uint(instance_count);

                let string = if array.data_type() == &DataType::UInt8 {
                    re_format::format_bytes(instance_count as _)
                } else if let Some(dtype) = simple_datatype_string(array.data_type()) {
                    format!("{instance_count_str} items of {dtype}")
                } else {
                    format!("{instance_count_str} items")
                };
                let response = ui_layout.label(ui, string);

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

// TODO(emilk): there is some overlap here with `re_format_arrow`.
fn simple_datatype_string(datatype: &DataType) -> Option<&'static str> {
    match datatype {
        DataType::Null => Some("null"),
        DataType::Boolean => Some("bool"),
        DataType::Int8 => Some("int8"),
        DataType::Int16 => Some("int16"),
        DataType::Int32 => Some("int32"),
        DataType::Int64 => Some("int64"),
        DataType::UInt8 => Some("uint8"),
        DataType::UInt16 => Some("uint16"),
        DataType::UInt32 => Some("uint32"),
        DataType::UInt64 => Some("uint64"),
        DataType::Float16 => Some("float16"),
        DataType::Float32 => Some("float32"),
        DataType::Float64 => Some("float64"),
        DataType::Utf8 | DataType::LargeUtf8 => Some("utf8"),
        DataType::Binary | DataType::LargeBinary => Some("binary"),
        _ => None,
    }
}
