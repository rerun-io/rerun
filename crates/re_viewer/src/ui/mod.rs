pub(crate) mod data_ui;
pub(crate) mod event_log_view;
pub(crate) mod kb_shortcuts;
pub(crate) mod selection_panel;
pub(crate) mod time_panel;

mod viewport;
pub(crate) use self::viewport::{Blueprint, SpaceViewId};

mod space_view;
use self::space_view::{SceneQuery, SpaceView};

pub(crate) mod view_2d;
pub(crate) mod view_3d;
mod view_tensor;
mod view_text_entry;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Preview {
    Small,
    Medium,
    Specific(f32),
}
