//! Things that should be upstream moved to egui/eframe at some point

pub mod card_layout;
mod completion;
pub mod garbage_collect;
mod response_ext;
mod shorten_text;
pub(crate) mod widget_ext;

pub use completion::{CompletionOutput, CompletionPopup, CompletionQuery, Suggestion};
pub use response_ext::ResponseExt;
pub use shorten_text::shorten_from_start;
