use crate::fmt::ArrayUi;
use arrow::array::Array;
use arrow::util::display::FormatOptions;

pub fn arrow_ui(ui: &mut egui::Ui, array: &dyn Array) {
    let formatter = ArrayUi::try_new(array, &FormatOptions::new()).unwrap();

    for i in 0..array.len() {
        formatter.show_value(i, ui);
    }
}
