use crate::fmt::{ArrayFormatter, FormatOptions};
use arrow::array::Array;

pub fn arrow_ui(ui: &mut egui::Ui, array: &dyn Array) {
    let formatter = ArrayFormatter::try_new(array, &FormatOptions::new()).unwrap();

    for i in 0..array.len() {
        formatter.show_value(i, ui);
    }
}
