use ahash::HashMap;
use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::{serde_field::SerdeField, Component};

pub use re_viewer_context::SpaceViewId;

use crate::ui::VisibilitySet;

#[derive(Clone, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ViewportComponent {
    #[arrow_field(type = "SerdeField<VisibilitySet>")]
    pub space_view_keys: std::collections::BTreeSet<SpaceViewId>,
    #[arrow_field(type = "SerdeField<VisibilitySet>")]
    pub visible: VisibilitySet,
    #[arrow_field(type = "SerdeField<HashMap<VisibilitySet, egui_dock::Tree<SpaceViewId>>>")]
    pub trees: HashMap<VisibilitySet, egui_dock::Tree<SpaceViewId>>,
    #[arrow_field(type = "Option<SerdeField<SpaceViewId>>")]
    pub maximized: Option<SpaceViewId>,
    pub has_been_user_edited: bool,
}

impl Default for ViewportComponent {
    fn default() -> Self {
        Self {
            space_view_keys: Default::default(),
            visible: Default::default(),
            trees: Default::default(),
            maximized: Default::default(),
            has_been_user_edited: true,
        }
    }
}

impl ViewportComponent {
    // TODO(jleibs): Can we make this an EntityPath instead?
    pub const VIEWPORT: &str = "viewport";
}

impl Component for ViewportComponent {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.viewport".into()
    }
}
