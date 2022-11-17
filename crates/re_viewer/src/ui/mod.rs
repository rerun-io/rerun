mod auto_layout;

mod viewport;
pub(crate) use self::viewport::{Blueprint, SpaceViewId};

mod space_view;
use self::space_view::SpaceView;

mod scene;
use self::scene::SceneQuery;

pub(crate) mod data_ui;
pub(crate) mod event_log_view;
pub(crate) mod kb_shortcuts;
pub(crate) mod selection_panel;
pub(crate) mod time_panel;

pub(crate) mod view_2d;
pub(crate) mod view_3d;
mod view_plot;
mod view_tensor;
mod view_text;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Preview {
    Small,
    Medium,
    Specific(f32),
}
