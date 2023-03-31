mod app_options;
pub mod caches;
pub mod format_time;
mod item;
pub(crate) mod mesh_loader;
pub mod queries;
mod selection_state;
pub(crate) mod space_info;
pub(crate) mod time_control;
pub(crate) mod time_control_ui;
mod transform_cache;
mod viewer_context;

pub use caches::Caches;

pub mod instance_hash_conversions;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

pub(crate) use time_control::{TimeControl, TimeView};
pub(crate) use viewer_context::*;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod profiler;

#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard;

pub use transform_cache::{TransformCache, UnreachableTransform};
pub use {
    app_options::*,
    item::{Item, ItemCollection},
    selection_state::{
        HoverHighlight, HoveredSpace, InteractionHighlight, OptionalSpaceViewEntityHighlight,
        SelectionHighlight, SelectionState, SpaceViewHighlights, SpaceViewOutlineMasks,
    },
};

// ----------------------------------------------------------------------------

pub fn help_hover_button(ui: &mut egui::Ui) -> egui::Response {
    ui.add(
        egui::Label::new("❓").sense(egui::Sense::click()), // sensing clicks also gives hover effect
    )
}
