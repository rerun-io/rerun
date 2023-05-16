use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::Component;

/// A Panel component
///
/// ## Example
/// ```
/// # use re_viewer::blueprint_components::panel::PanelState;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     PanelState::data_type(),
///     DataType::Struct(vec![
///         Field::new("expanded", DataType::Boolean, false),
///     ])
/// );
/// ```
// TODO(jleibs): If we want these accessible from python, they need to
// go into the registry that's back in `re_log_types`
#[derive(Debug, Clone, Copy, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct PanelState {
    pub expanded: bool,
}

impl PanelState {
    // TODO(jleibs): Would be nice if this could be a const EntityPath but making
    // the hash const is a bit of a pain.
    pub const BLUEPRINT_VIEW_PATH: &str = "blueprint_view";
    pub const SELECTION_VIEW_PATH: &str = "selection_view";
    pub const TIMELINE_VIEW_PATH: &str = "timeline_view";
}

impl Component for PanelState {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.panel_view".into()
    }
}
