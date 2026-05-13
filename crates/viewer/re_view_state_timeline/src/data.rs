/// Collection of state change lanes produced by a visualizer.
#[derive(Clone, Debug, Default)]
pub struct StateLanesData {
    pub lanes: Vec<StateLane>,
}

/// A single horizontal lane of state change phases.
#[derive(Clone, Debug)]
pub struct StateLane {
    /// Display name for this lane (typically the entity path).
    pub label: String,

    /// The entity path this lane belongs to.
    pub entity_path: re_log_types::EntityPath,

    /// Ordered list of phases. Each phase starts at `start_time` and implicitly ends
    /// where the next phase begins (or at the right edge of the visible range).
    pub phases: Vec<StateLanePhase>,
}

/// One contiguous phase within a [`StateLane`].
#[derive(Clone, Debug)]
pub struct StateLanePhase {
    /// Start time in timeline units.
    pub start_time: i64,

    /// Human-readable state label (e.g. "Idle", "Moving").
    pub label: String,

    /// Display color for this phase.
    pub color: egui::Color32,

    /// Whether this phase should be drawn.
    pub visible: bool,
}
