//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all views.

mod system_execution;
mod viewport_ui;

pub use viewport_ui::ViewportUi;

pub mod external {
    pub use re_sdk_types;
}

// TODO(andreas): cfg test this only?
