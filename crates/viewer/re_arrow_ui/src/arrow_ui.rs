use crate::datatype_ui::data_type_ui;
use crate::fmt::ArrayUi;
use crate::fmt_ui;
use arrow::array::AsArray;
use arrow::{
    array::Array,
    datatypes::DataType,
    error::ArrowError,
    util::display::{ArrayFormatter, FormatOptions},
};
use egui::text::LayoutJob;
use egui::{Id, Response, RichText, Stroke, StrokeKind, TextFormat, TextStyle, Ui, WidgetText};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_ui::list_item::{LabelContent, PropertyContent, list_item_scope};
use re_ui::{UiExt, UiLayout};
use std::ops::Range;

pub fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow::array::Array) {
    re_tracing::profile_function!();

    use arrow::array::{LargeListArray, LargeStringArray, ListArray, StringArray};

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

        // TODO: I think we can remove this since the custom code should never render more than a couple items in a list.
        // Special-treat batches that are themselves unit-lists (i.e. blobs).
        //
        // What we really want to display in these instances in the underlying array, otherwise we'll
        // bring down the entire viewer trying to render a list whose single entry might itself be
        // an array with millions of values.
        // if let Some(entries) = array.downcast_array_ref::<ListArray>() {
        //     if entries.len() == 1 {
        //         // Don't use `values` since this may contain values before and after the single blob we want to display.
        //         return arrow_ui(ui, ui_layout, &entries.value(0));
        //     }
        // }
        // if let Some(entries) = array.downcast_array_ref::<LargeListArray>() {
        //     if entries.len() == 1 {
        //         // Don't use `values` since this may contain values before and after the single blob we want to display.
        //         return arrow_ui(ui, ui_layout, &entries.value(0));
        //     }
        // }

        match make_ui(array) {
            Ok(array_formatter) => match ui_layout {
                UiLayout::SelectionPanel => {
                    list_item_scope(ui, Id::new("arrow_ui"), |ui| {
                        let (name, maybe_data_type_ui) = data_type_ui(array.data_type());
                        let content = PropertyContent::new("Data type")
                            .value_text(name)
                            .show_only_when_collapsed(false);
                        if let Some(datatype_ui) = maybe_data_type_ui {
                            ui.list_item().show_hierarchical_with_children(
                                ui,
                                Id::new("data_type_ui_root"),
                                false,
                                content,
                                datatype_ui,
                            );
                        } else {
                            ui.list_item().show_hierarchical(ui, content);
                        }
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

pub(crate) fn make_ui(array: &dyn Array) -> Result<ArrayUi, ArrowError> {
    let options = FormatOptions::default()
        .with_null("null")
        .with_display_error(true);
    let array_ui = ArrayUi::try_new(array, &options)?;
    Ok(array_ui)
}
