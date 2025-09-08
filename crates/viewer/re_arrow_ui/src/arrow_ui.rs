use arrow::{array::Array, error::ArrowError, util::display::FormatOptions};
use re_arrow_util::ArrayCellRef;
use re_ui::list_item::list_item_scope;
use re_ui::{UiExt as _, UiLayout};

//use crate::ScalarRef;
use crate::datatype_ui::DataTypeUi;
use crate::show_index::ArrayUi;

/// This controls whether we strip the `[]` when displaying non-scalar arrays of length 1.
///
/// Doing so keeps the display less busy for our mono-component data. Not doing so is more correct
/// and consistent.
///
/// Note: changing this value invalidates hundreds of UI snapshots.
const UNWRAP_MONO_ARRAY: bool = false; //TODO: revert

// pub enum ArrayMay

pub fn arrow_ui<'a>(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    array_cell: impl Into<ArrayCellRef<'a>>,
) {
    re_tracing::profile_function!();

    // keep monomorphization in check
    fn arrow_ui_inner(
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        array: &dyn Array,
        unwrap_mono_array: bool,
    ) {
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
                            if unwrap_mono_array && array.len() == 1 {
                                array_formatter.show_value(0, ui);
                            } else {
                                array_formatter.show(ui);
                            }
                        });
                    }
                    UiLayout::Tooltip | UiLayout::List => {
                        let highlighted = if unwrap_mono_array && array.len() == 1 {
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

    let array_cell = array_cell.into();

    let (array, unwrap_mono_array) = match array_cell {
        ArrayCellRef::Array(array) => (array, UNWRAP_MONO_ARRAY),
        ArrayCellRef::Scalar(array) => (array, true),
    };

    arrow_ui_inner(ui, ui_layout, array, unwrap_mono_array);
}

pub(crate) fn make_ui(array: &dyn Array) -> Result<ArrayUi<'_>, ArrowError> {
    let options = FormatOptions::default()
        .with_null("null")
        .with_display_error(true);
    let array_ui = ArrayUi::try_new(array, &options)?;
    Ok(array_ui)
}
