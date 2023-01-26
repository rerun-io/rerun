mod annotations;
mod auto_layout;
mod blueprint;
mod data_blueprint;
mod scene;
mod selection_history;
mod selection_history_ui;
mod space_view;
mod view_bar_chart;
mod view_category;
mod view_tensor;
mod view_text;
mod view_time_series;
mod viewport;

pub(crate) mod data_ui;
pub(crate) mod event_log_view;
pub(crate) mod memory_panel;
pub(crate) mod selection_panel;
pub(crate) mod time_panel;

pub mod view_spatial;

// ----

use self::scene::SceneQuery;

pub(crate) use self::blueprint::Blueprint;
pub(crate) use self::space_view::{SpaceView, SpaceViewId};

pub use self::annotations::{Annotations, DefaultColor, MISSING_ANNOTATIONS};
pub use self::data_blueprint::DataBlueprintGroupHandle;
pub use self::selection_history::{HistoricalSelection, SelectionHistory};
pub use self::viewport::Viewport;

pub(crate) use data_ui::Preview;
use re_log_types::FieldOrComponent;

pub fn format_component_name(name: &re_data_store::FieldName) -> String {
    let name = name.as_str();
    if let Some(name) = name.strip_prefix("rerun.") {
        name.into()
    } else {
        name.into()
    }
}

pub fn format_field_or_component_name(name: &FieldOrComponent) -> String {
    match name {
        FieldOrComponent::Field(name) | FieldOrComponent::Component(name) => {
            format_component_name(name)
        }
    }
}
