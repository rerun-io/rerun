use crate::datatype_ui::DataTypeUi;
use crate::show_index::ArrayUi;
use arrow::{array::Array, error::ArrowError, util::display::FormatOptions};
use egui::Id;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_ui::list_item::{PropertyContent, list_item_scope};
use re_ui::{UiExt as _, UiLayout};

pub fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn Array) {
    re_tracing::profile_function!();

    use arrow::array::{LargeStringArray, StringArray};

    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        // TODO: Should this also be handled in the arrow tree UI?
        // Special-treat text.
        // This is so that we can show urls as clickable links.
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

        match make_ui(array) {
            Ok(array_formatter) => match ui_layout {
                UiLayout::SelectionPanel => {
                    list_item_scope(ui, Id::new("arrow_ui"), |ui| {
                        DataTypeUi::new(array.data_type()).list_item_ui(ui);
                        if array.len() == 1 {
                            array_formatter.show_value(0, ui);
                        } else {
                            array_formatter.show(ui);
                        }
                    });
                }
                UiLayout::Tooltip | UiLayout::List => {
                    let job = if array.len() == 1 {
                        array_formatter.value_job(ui, 0)
                    } else {
                        array_formatter.job(ui)
                    };
                    match job {
                        Ok(job) => {
                            ui_layout.label(ui, job);
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

pub(crate) fn make_ui(array: &dyn Array) -> Result<ArrayUi<'_>, ArrowError> {
    let options = FormatOptions::default()
        .with_null("null")
        .with_display_error(true);
    let array_ui = ArrayUi::try_new(array, &options)?;
    Ok(array_ui)
}
