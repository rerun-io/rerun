//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all views.

mod system_execution;
mod view_highlights;
mod view_loading_indicator;
mod viewport_ui;

pub use view_loading_indicator::paint_view_loading_indicator;
pub use viewport_ui::ViewportUi;

pub mod external {
    pub use re_sdk_types;
}

// TODO(andreas): cfg test this only?
pub use system_execution::{execute_systems_for_view, new_view_query};
