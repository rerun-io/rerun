use arrow::array::Array;
use arrow::error::ArrowError;
use re_format::TimestampFormat;
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

    // Should always be able to copy out raw arrow values!
    let previous_selectable_labels = ui.style().interaction.selectable_labels;
    ui.style_mut().interaction.selectable_labels = true;

    let previous_wrap_mode = ui.style().wrap_mode;
    if ui_layout != UiLayout::Inline {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    }

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
            UiLayout::Tooltip | UiLayout::List | UiLayout::Inline => {
                let highlighted = if array.len() == 1 {
                    array_formatter.value_highlighted(0)
                } else {
                    array_formatter.highlighted()
                };
                match highlighted {
                    Ok(job) => {
                        if ui_layout == UiLayout::Tooltip || ui.is_tooltip() {
                            ui_layout.data_label(ui, job);
                        } else {
                            ui_layout.data_label_with_details(ui, &job, |ui| {
                                details_ui(ui, timestamp_format, array);
                            });
                        }
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
    ui.style_mut().wrap_mode = previous_wrap_mode;
    ui.style_mut().interaction.selectable_labels = previous_selectable_labels;
}

/// The full tree of `array`, shown on hover when its one-line summary leaves something out.
fn details_ui(ui: &mut egui::Ui, timestamp_format: TimestampFormat, array: &dyn Array) {
    /// Taller values scroll, so that every row stays reachable.
    const MAX_HEIGHT: f32 = 400.0;

    let array_formatter = match make_ui(timestamp_format, array) {
        Ok(array_formatter) => array_formatter,
        Err(err) => {
            ui.error_with_details_on_hover(err.to_string());
            return;
        }
    };

    // A single leaf value (e.g. a long string) has no tree to show, so show all of its text.
    if array.len() == 1 && !array_formatter.is_item_nested() {
        match array_formatter.value_highlighted(0) {
            Ok(job) => {
                egui::ScrollArea::vertical()
                    .max_height(MAX_HEIGHT)
                    .show(ui, |ui| UiLayout::SelectionPanel.data_label(ui, job));
            }
            Err(err) => {
                ui.error_with_details_on_hover(err.to_string());
            }
        }
        return;
    }

    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    ui.set_width(ui.spacing().tooltip_width);

    egui::ScrollArea::vertical()
        .max_height(MAX_HEIGHT)
        .show(ui, |ui| {
            list_item_scope(ui, "arrow_ui_details", |ui| {
                if array.len() == 1 {
                    array_formatter.show_value(0, ui);
                } else {
                    array_formatter.show(ui);
                }
            });
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
            max_string_chars: if array.len() > 1 { 100 } else { usize::MAX },
            ..Default::default()
        },
    )?;
    Ok(array_ui)
}
