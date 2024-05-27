//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

mod add_space_view_or_container_modal;
mod auto_layout;
mod screenshot;
mod space_view_entity_picker;
pub mod space_view_heuristics;
mod space_view_highlights;
mod system_execution;
mod viewport;
mod viewport_blueprint_ui;

pub use self::viewport::{Viewport, ViewportState};
pub use self::viewport_blueprint_ui::contents_name_style;

pub mod external {
    pub use re_space_view;
    pub use re_types_blueprint;
}

// ---

/// Determines the icon to use for a given container kind.
#[inline]
pub fn icon_for_container_kind(kind: &egui_tiles::ContainerKind) -> &'static re_ui::Icon {
    match kind {
        egui_tiles::ContainerKind::Tabs => &re_ui::icons::CONTAINER_TABS,
        egui_tiles::ContainerKind::Horizontal => &re_ui::icons::CONTAINER_HORIZONTAL,
        egui_tiles::ContainerKind::Vertical => &re_ui::icons::CONTAINER_VERTICAL,
        egui_tiles::ContainerKind::Grid => &re_ui::icons::CONTAINER_GRID,
    }
}
