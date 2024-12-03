pub(super) mod add_container;
pub(super) mod add_entities_to_new_space_view;
pub(super) mod add_space_view;
pub(super) mod clone_space_view;
pub(super) mod collapse_expand_all;
pub(super) mod move_contents_to_new_container;
pub(super) mod remove;
pub(super) mod show_hide;

#[cfg(not(target_arch = "wasm32"))] // TODO(#8264): screenshotting on web
mod screenshot_action;

#[cfg(not(target_arch = "wasm32"))] // TODO(#8264): screenshotting on web
pub use screenshot_action::ScreenshotAction;
