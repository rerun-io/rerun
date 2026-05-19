/// Collection of status lanes produced by a visualizer.
#[derive(Clone, Debug, Default)]
pub struct StatusLanesData {
    pub lanes: Vec<StatusLane>,
}

/// A single horizontal lane of status phases.
#[derive(Clone, Debug)]
pub struct StatusLane {
    /// Display name for this lane (typically the entity path).
    pub label: String,

    /// Ordered list of phases. Each phase starts at `start_time` and implicitly ends
    /// where the next phase begins (or at the right edge of the visible range).
    pub phases: Vec<StatusLanePhase>,
}

/// One contiguous phase within a [`StatusLane`].
#[derive(Clone, Debug)]
pub struct StatusLanePhase {
    /// Start time in timeline units.
    pub start_time: i64,

    /// Human-readable status label (e.g. "Idle", "Moving").
    pub label: String,

    /// Display color for this phase.
    pub color: egui::Color32,
}
