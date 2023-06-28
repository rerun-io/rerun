//! Potentially user-facing components to be used in blueprints.

mod space_view;
mod viewport;

pub use space_view::SpaceViewComponent;
pub use viewport::{AutoSpaceViews, SpaceViewMaximized, ViewportLayout, VIEWPORT_PATH};
