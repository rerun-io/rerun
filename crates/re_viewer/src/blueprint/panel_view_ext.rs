use super::PanelView;

impl PanelView {
    // TODO(jleibs): Would be nice if this could be a const EntityPath but making
    // the hash const is a bit of a pain.
    pub const BLUEPRINT_VIEW_PATH: &'static str = "blueprint_view";
    pub const SELECTION_VIEW_PATH: &'static str = "selection_view";
    pub const TIMELINE_VIEW_PATH: &'static str = "timeline_view";
}
