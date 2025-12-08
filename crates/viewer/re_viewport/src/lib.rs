//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all views.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod system_execution;
mod view_highlights;
mod viewport_ui;

pub use viewport_ui::ViewportUi;

pub mod external {
    pub use {re_sdk_types, re_view};
}

// TODO(andreas): cfg test this only?
pub use system_execution::{execute_systems_for_view, new_view_query};
