//! Things that should be upstream moved to egui/eframe at some point

pub mod card_layout;
mod group;
mod kb_shortcut_ext;
pub mod response_ext;
pub(crate) mod widget_ext;
mod widget_text_ext;

pub use group::Group;
pub use kb_shortcut_ext::KeyboardShortcutExt;
pub use widget_text_ext::concat_rich_text;
