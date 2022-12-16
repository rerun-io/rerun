mod annotations;
mod auto_layout;
mod blueprint;
mod class_description_ui;
mod image_ui;
mod scene;
mod selection_history;
mod selection_history_ui;
mod space_view;
mod transform_cache;
mod view_category;
mod view_plot;
mod view_tensor;
mod view_text;
mod viewport;

pub(crate) mod data_ui;
pub(crate) mod event_log_view;
pub(crate) mod kb_shortcuts;
pub(crate) mod memory_panel;
pub(crate) mod selection_panel;
pub(crate) mod time_panel;

pub mod view_spatial;

// ----

use self::scene::SceneQuery;

pub(crate) use self::blueprint::Blueprint;
pub(crate) use self::space_view::{SpaceView, SpaceViewId};

pub use self::annotations::{Annotations, DefaultColor};
pub use self::selection_history::{HistoricalSelection, SelectionHistory};

// ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Preview {
    Small,
    Medium,
    Specific(f32),
}
