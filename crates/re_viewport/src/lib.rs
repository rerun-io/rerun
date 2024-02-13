//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

pub const VIEWPORT_PATH: &str = "viewport";

mod add_space_view_or_container_modal;
mod auto_layout;
mod container;
mod context_menu;
mod screenshot;
mod space_view_entity_picker;
pub mod space_view_heuristics;
mod space_view_highlights;
mod system_execution;
mod viewport;
mod viewport_blueprint;
mod viewport_blueprint_ui;

/// Auto-generated blueprint-related types.
///
/// They all implement the [`re_types_core::Component`] trait.
///
/// Unstable. Used for the ongoing blueprint experimentation.
pub mod blueprint;

pub use container::{ContainerBlueprint, Contents};
pub use context_menu::context_menu_ui_for_item;
pub use viewport::{Viewport, ViewportState};
pub use viewport_blueprint::ViewportBlueprint;
pub use viewport_blueprint_ui::space_view_name_style;

pub mod external {
    pub use re_space_view;
}

use re_types::datatypes;

// TODO(andreas): Workaround for referencing non-blueprint components from blueprint archetypes.
pub(crate) mod components {
    pub use re_types::components::Name;
}

/// Determines the icon to use for a given container kind.
pub fn icon_for_container_kind(kind: &egui_tiles::ContainerKind) -> &'static re_ui::Icon {
    match kind {
        egui_tiles::ContainerKind::Tabs => &re_ui::icons::CONTAINER_TABS,
        egui_tiles::ContainerKind::Horizontal => &re_ui::icons::CONTAINER_HORIZONTAL,
        egui_tiles::ContainerKind::Vertical => &re_ui::icons::CONTAINER_VERTICAL,
        egui_tiles::ContainerKind::Grid => &re_ui::icons::CONTAINER_GRID,
    }
}
