use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::Component;

/// A Panel component
// TODO(jleibs): If we want these accessible from python, they need to
// go into the registry that's back in `re_log_types`
#[derive(Debug, Clone, Copy, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct PanelState {
    pub expanded: bool,
}

impl PanelState {
    // TODO(jleibs): Would be nice if this could be a const EntityPath but making
    // the hash const is a bit of a pain.
    pub const BLUEPRINT_VIEW: &str = "blueprint_view";
    pub const SELECTION_VIEW: &str = "selection_view";
    pub const TIMELINE_VIEW: &str = "timeline_view";
}

impl Component for PanelState {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.panel_view".into()
    }
}
