pub(crate) mod class_description_ui;
pub(crate) mod data_ui;
pub(crate) mod event_log_view;
pub(crate) mod image_ui;
pub(crate) mod kb_shortcuts;
pub(crate) mod selection_panel;
pub(crate) mod space_view;
pub(crate) mod time_panel;
pub(crate) mod view2d;
pub(crate) mod viewport;

pub(crate) use space_view::SpaceView;
pub(crate) use viewport::{Blueprint, SpaceViewId};

pub(crate) mod view_3d;
mod view_tensor;
mod view_text_entry;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Preview {
    Small,
    Medium,
    Specific(f32),
}

// ---

// TODO

mod legend;
pub use self::legend::{ClassDescription, ClassDescriptionMap, ColorMapping, Legend, Legends};
