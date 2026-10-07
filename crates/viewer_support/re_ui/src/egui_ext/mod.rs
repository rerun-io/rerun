//! Things that should be upstream moved to egui/eframe at some point

pub mod card_layout;
mod completion;
pub mod garbage_collect;
pub(crate) mod widget_ext;

pub use completion::{CompletionOutput, CompletionPopup, CompletionQuery, Suggestion};
