use crate::fmt::ArrayUi;
use arrow::array::Array;
use arrow::error::ArrowError;
use arrow::util::display::FormatOptions;
use egui::Widget;
use re_ui::UiExt;

pub fn arrow_tree_ui(ui: &mut egui::Ui, array: &dyn Array) {
    let formatter = ArrayUi::try_new(array, &FormatOptions::new());
    match formatter {
        Ok(formatter) => {
            formatter.show(ui);
        }
        Err(err) => {
            ui.error_label(err.to_string());
        }
    }
}
