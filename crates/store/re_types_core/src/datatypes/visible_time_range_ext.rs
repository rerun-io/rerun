use super::{TimeRange, VisibleTimeRange};

impl Default for VisibleTimeRange {
    fn default() -> Self {
        // Actual defaults used in the viewer differ per view.
        Self {
            timeline: "log_time".into(),
            range: TimeRange::EVERYTHING,
        }
    }
}
