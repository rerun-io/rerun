use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::Component;

/// A Panel component
/// TODO(jleibs): If we want these accessible from python, they need to
/// go into the registry that's back in `re_log_types`
#[derive(Debug, Clone, Copy, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct PanelState {
    pub expanded: bool,
}

impl PanelState {
    // TODO(jleibs): Would be nice if this could be a const EntityPath but making
    // the hash const is a bit of a pain.
    pub const BLUEPRINT_PANEL: &str = "blueprint_panel";
    pub const SELECTION_PANEL: &str = "selection_panel";
    pub const TIMELINE_PANEL: &str = "timeline_panel";
}

impl Component for PanelState {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.panel".into()
    }
}
