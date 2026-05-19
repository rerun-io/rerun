//! Things that should be upstream moved to egui/eframe at some point

pub mod boxed_widget;
pub mod card_layout;
mod group;
pub mod response_ext;
pub(crate) mod widget_ext;
mod widget_text_ext;

pub use group::Group;
pub use widget_text_ext::WidgetTextExt;
