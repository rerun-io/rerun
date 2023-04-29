//! Potentially user-facing components to be used in blueprints.
//! TODO(jleibs): These should live in their own crate so we don't need a
//!               viewer dep in order to make use of them.
mod panel;
mod space_view;

pub use panel::PanelState;
