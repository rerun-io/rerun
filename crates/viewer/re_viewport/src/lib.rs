//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all views.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod auto_layout;
mod system_execution;
mod view_highlights;
mod viewport_ui;

pub use self::viewport_ui::ViewportUi;

pub mod external {
    pub use re_types;
    pub use re_view;
}
