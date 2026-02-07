use arrow::array::Array;
use arrow::error::ArrowError;
use re_log_types::TimestampFormat;
use re_ui::list_item::list_item_scope;
use re_ui::{UiExt as _, UiLayout};

use crate::datatype_ui::DataTypeUi;
use crate::show_index::{ArrayUi, DisplayOptions};

pub fn arrow_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    timestamp_format: TimestampFormat,
    array: &dyn Array,
) {
    re_tracing::profile_function!();

    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        match make_ui(timestamp_format, array) {
            Ok(array_formatter) => match ui_layout {
                UiLayout::SelectionPanel => {
                    // Data type has a separate scope to prevent items from being aligned.
                    // If they are aligned it makes it confusingly look like a table.
                    list_item_scope(ui, "arrow_data_type_ui", |ui| {
                        DataTypeUi::new(array.data_type()).list_item_ui(ui);
                    });
                    list_item_scope(ui, "arrow_ui", |ui| {
                        if array.len() == 1 {
                            array_formatter.show_value(0, ui);
                        } else {
                            array_formatter.show(ui);
                        }
                    });
                }
                UiLayout::Tooltip | UiLayout::List => {
                    let highlighted = if array.len() == 1 {
                        array_formatter.value_highlighted(0)
                    } else {
                        array_formatter.highlighted()
                    };
                    match highlighted {
                        Ok(job) => {
                            ui_layout.data_label(ui, job);
                        }
                        Err(err) => {
                            ui.error_with_details_on_hover(err.to_string());
                        }
                    }
                }
            },
            Err(err) => {
                ui.error_with_details_on_hover(err.to_string());
            }
        }
    });
}

pub(crate) fn make_ui(
    timestamp_format: TimestampFormat,
    array: &dyn Array,
) -> Result<ArrayUi<'_>, ArrowError> {
    let array_ui = ArrayUi::try_new(
        array,
        &DisplayOptions {
            timestamp_format,
            ..Default::default()
        },
    )?;
    Ok(array_ui)
}
