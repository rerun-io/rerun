mod annotations;
mod auto_layout;
mod blueprint;
mod data_blueprint;
mod scene;
mod selection_history;
mod selection_history_ui;
mod space_view;
mod space_view_entity_picker;
mod space_view_heuristics;
mod view_bar_chart;
mod view_category;
mod view_node_graph;
mod view_tensor;
mod view_text;
mod view_time_series;
mod viewport;
pub(crate) mod bottom_panel;

pub(crate) mod data_ui;
pub(crate) mod device_settings_panel;
pub(crate) mod memory_panel;
pub(crate) mod selection_panel;
pub(crate) mod stats_panel;
pub(crate) mod time_panel;

pub mod view_spatial;

// ----

use self::scene::SceneQuery;

pub(crate) use self::blueprint::Blueprint;
pub(crate) use self::space_view::{SpaceView, SpaceViewId, SpaceViewKind};

pub use self::annotations::{Annotations, DefaultColor, MISSING_ANNOTATIONS};
pub use self::data_blueprint::DataBlueprintGroupHandle;
pub use self::selection_history::{HistoricalSelection, SelectionHistory};
pub use self::view_category::ViewCategory;
pub use self::viewport::Viewport;

pub(crate) use data_ui::UiVerbosity;
