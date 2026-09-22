//! Things that should be upstream moved to egui/eframe at some point

pub mod card_layout;
mod completion;
pub mod garbage_collect;
mod group;
pub(crate) mod widget_ext;
mod widget_text_ext;

pub use completion::{CompletionOutput, CompletionPopup, CompletionQuery, Suggestion};
pub use group::Group;
pub use widget_text_ext::concat_rich_text;
