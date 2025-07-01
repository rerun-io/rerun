//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all views.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod system_execution;
mod view_highlights;
mod viewport_ui;

#[cfg(feature = "testing")]
pub mod test_context_ext;

pub use self::viewport_ui::ViewportUi;

pub mod external {
    pub use re_types;
    pub use re_view;
}

// TODO(andreas): cfg test this only?
pub use system_execution::execute_systems_for_view;
