//! Potentially user-facing components to be used in blueprints.

mod space_view_component;
mod viewport;

pub use self::space_view_component::SpaceViewComponent;
pub use self::viewport::{SpaceViewMaximized, ViewportLayout, VIEWPORT_PATH};
