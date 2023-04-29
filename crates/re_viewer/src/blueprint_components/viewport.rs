use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::{serde_field::SerdeField, Component};

use crate::ui::SpaceViewId;

// TODO(jleibs) export this from other viewport def
type VisibilitySet = std::collections::BTreeSet<SpaceViewId>;

#[derive(Clone, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ViewportComponent {
    #[arrow_field(type = "SerdeField<VisibilitySet>")]
    pub visible: VisibilitySet,
    // TODO(jleibs): Something down in arrow-convert still requires implementing support for `==`
    // Since we're replacing this with our own layout anyways, remove this for now
    //#[arrow_field(type = "SerdeField<HashMap<VisibilitySet, egui_dock::Tree<SpaceViewId>>>")]
    //trees: HashMap<VisibilitySet, egui_dock::Tree<SpaceViewId>>,
    #[arrow_field(type = "Option<SerdeField<SpaceViewId>>")]
    pub maximized: Option<SpaceViewId>,
    pub has_been_user_edited: bool,
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
