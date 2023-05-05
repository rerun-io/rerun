pub mod format_time;
mod mesh_cache;
pub(crate) mod mesh_loader;
pub mod queries;
pub(crate) mod space_info;
mod space_view_highlights;
mod time_control_ui;
mod transform_cache;

pub mod instance_hash_conversions;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod profiler;

pub use mesh_cache::MeshCache;
pub use time_control_ui::TimeControlUi;
pub use transform_cache::{TransformCache, UnreachableTransform};

pub use space_view_highlights::{
    highlights_for_space_view, OptionalSpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewOutlineMasks,
};

// ----------------------------------------------------------------------------

pub fn help_hover_button(ui: &mut egui::Ui) -> egui::Response {
    ui.add(
        egui::Label::new("❓").sense(egui::Sense::click()), // sensing clicks also gives hover effect
    )
}
