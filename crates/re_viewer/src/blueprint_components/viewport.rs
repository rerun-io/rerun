use ahash::HashMap;
use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::{serde_field::SerdeField, Component};

pub use re_viewer_context::SpaceViewId;

use crate::ui::VisibilitySet;

pub const VIEWPORT_PATH: &str = "viewport";

#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct AutoSpaceViews(pub bool);

impl Component for AutoSpaceViews {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.auto_space_views".into()
    }
}

#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct SpaceViewVisibility(
    #[arrow_field(type = "SerdeField<VisibilitySet>")] pub VisibilitySet,
);

impl Component for SpaceViewVisibility {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.space_view_visibility".into()
    }
}

#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct SpaceViewMaximized(
    #[arrow_field(type = "Option<SerdeField<SpaceViewId>>")] pub Option<SpaceViewId>,
);

impl Component for SpaceViewMaximized {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.maximized".into()
    }
}

#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ViewportLayout {
    #[arrow_field(type = "SerdeField<std::collections::BTreeSet<SpaceViewId>>")]
    pub space_view_keys: std::collections::BTreeSet<SpaceViewId>,
    #[arrow_field(type = "SerdeField<HashMap<VisibilitySet, egui_tiles::Tree<SpaceViewId>>>")]
    pub trees: HashMap<VisibilitySet, egui_tiles::Tree<SpaceViewId>>,
    pub has_been_user_edited: bool,
}

impl Component for ViewportLayout {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.viewport_layout".into()
    }
}
