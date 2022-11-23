pub(crate) mod color_map;
pub(crate) mod mesh_loader;
pub(crate) mod space_info;
pub(crate) mod tensor_image_cache;
pub(crate) mod time_axis;
pub(crate) mod time_control;
pub(crate) mod time_control_ui;
mod time_range;
mod time_real;
mod viewer_context;

use tensor_image_cache::ImageCache;

mod path_browser;
pub use self::path_browser::PathBrowser;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

pub(crate) use time_control::{TimeControl, TimeView};
pub(crate) use time_range::{TimeRange, TimeRangeF};
pub(crate) use time_real::TimeReal;
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
