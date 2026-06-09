/// Collection of state change lanes produced by a visualizer.
#[derive(Clone, Debug, Default)]
pub struct StateLanesData {
    pub lanes: Vec<StateLane>,
}

/// Canonical post-cast value type of a state lane.
///
/// The polymorphic state cast collapses every accepted source type into one of these — strings
/// stay as strings, booleans stay as booleans, all numeric types collapse to `Scalar` (Float64).
/// The configuration editor branches on this to offer a type-appropriate UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateValueKind {
    String,
    Scalar,
    Bool,
}

/// A single horizontal lane of state change phases.
#[derive(Clone, Debug)]
pub struct StateLane {
    /// Display name for this lane (typically the entity path).
    pub label: String,

    /// The entity path this lane belongs to.
    pub entity_path: re_log_types::EntityPath,

    /// The canonical post-cast type of the values in this lane.
    pub value_kind: StateValueKind,

    /// Ordered list of phases. Each phase starts at `start_time` and implicitly ends
    /// where the next phase begins (or at the right edge of the visible range).
    pub phases: Vec<StateLanePhase>,
}

/// One contiguous phase within a [`StateLane`].
#[derive(Clone, Debug)]
pub struct StateLanePhase {
    /// Start time in timeline units.
    pub start_time: i64,

    /// `Some` for a drawn state; `None` for a gap region or invisible state.
    pub content: Option<StateLanePhaseContent>,
}

/// Visual style for a drawn state phase.
#[derive(Clone, Debug)]
pub struct StateLanePhaseContent {
    /// Human-readable state label (e.g. "Idle", "Moving").
    pub label: String,

    /// Display color for this phase.
    pub color: egui::Color32,
}
