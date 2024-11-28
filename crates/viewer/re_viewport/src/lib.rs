//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod auto_layout;
mod screenshot;
mod space_view_highlights;
mod system_execution;
mod viewport_ui;

pub use self::viewport_ui::ViewportUi;

pub mod external {
    pub use re_space_view;
    pub use re_types_blueprint;
}
