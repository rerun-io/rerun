pub mod caches;
pub(crate) mod color_map;
pub(crate) mod mesh_loader;
pub(crate) mod space_info;
pub(crate) mod time_axis;
pub(crate) mod time_control;
pub(crate) mod time_control_ui;
mod viewer_context;

pub use caches::Caches;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

pub(crate) use time_control::{TimeControl, TimeView};
pub(crate) use viewer_context::*;

#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub(crate) mod profiler;

#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard;

// ----------------------------------------------------------------------------

pub fn help_hover_button(ui: &mut egui::Ui) -> egui::Response {
    ui.add(
        egui::Label::new("❓").sense(egui::Sense::click()), // sensing clicks also gives hover effect
    )
}

/// Recursively draw a tree for the [`arrow2::datatypes::DataType`]
pub fn data_type_tree(data_type: &arrow2::datatypes::DataType, expanded: bool, ui: &mut egui::Ui) {
    match data_type {
        arrow2::datatypes::DataType::List(field) => {
            egui::CollapsingHeader::new(format!("List ({})", field.name))
                .id_source(field)
                .default_open(expanded)
                .show(ui, |ui| {
                    data_type_tree(&field.data_type, expanded, ui);
                });
        }
        arrow2::datatypes::DataType::FixedSizeList(_, _) => todo!(),
        arrow2::datatypes::DataType::LargeList(_) => todo!(),
        arrow2::datatypes::DataType::Struct(fields) => {
            egui::CollapsingHeader::new("Struct")
                .id_source(fields)
                .default_open(expanded)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        for field in fields {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}: ", field.name));
                                data_type_tree(field.data_type(), expanded, ui);
                            });
                        }
                    });
                });
        }
        arrow2::datatypes::DataType::Union(_, _, _) => todo!(),
        arrow2::datatypes::DataType::Map(_, _) => todo!(),
        arrow2::datatypes::DataType::Dictionary(_, _, _) => todo!(),
        arrow2::datatypes::DataType::Extension(_, _, _) => todo!(),
        _ => {
            ui.label(format!("{:?}", data_type));
        }
    }
}
