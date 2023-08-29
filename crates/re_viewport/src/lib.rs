//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

mod auto_layout;
mod space_info;
mod space_view;
mod space_view_entity_picker;
mod space_view_heuristics;
mod space_view_highlights;
mod viewport;
mod viewport_blueprint;
mod viewport_blueprint_ui;

pub mod blueprint_components;

pub use space_info::SpaceInfoCollection;
pub use space_view::SpaceViewBlueprint;
pub use viewport::{Viewport, ViewportState};
pub use viewport_blueprint::ViewportBlueprint;

pub mod external {
    pub use re_space_view;
}
