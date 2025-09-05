use arrow::{array::Array, error::ArrowError, util::display::FormatOptions};
use re_ui::list_item::list_item_scope;
use re_ui::{UiExt as _, UiLayout};

use crate::datatype_ui::DataTypeUi;
use crate::show_index::ArrayUi;

pub fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn Array) {
    re_tracing::profile_function!();

    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        match make_ui(array) {
            Ok(array_formatter) => match ui_layout {
                UiLayout::SelectionPanel => {
                    // Data type has a separate scope to prevent items from being aligned.
                    // If they are aligned it makes it confusingly look like a table.
                    list_item_scope(ui, "arrow_data_type_ui", |ui| {
                        DataTypeUi::new(array.data_type()).list_item_ui(ui);
                    });
                    list_item_scope(ui, "arrow_ui", |ui| {
                        array_formatter.show(ui);
                    });
                }
                UiLayout::Tooltip | UiLayout::List => {
                    let highlighted = array_formatter.highlighted();
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

pub(crate) fn make_ui(array: &dyn Array) -> Result<ArrayUi<'_>, ArrowError> {
    let options = FormatOptions::default()
        .with_null("null")
        .with_display_error(true);
    let array_ui = ArrayUi::try_new(array, &options)?;
    Ok(array_ui)
}
