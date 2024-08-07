use re_log_types::TimelineName;

use super::Timeline;

impl Timeline {
    /// Return the name of the timeline.
    pub fn timeline_name(&self) -> TimelineName {
        TimelineName::from(self.0.as_str())
    }

    /// Set the name of the timeline.
    pub fn set_timeline_name(&mut self, timeline_name: TimelineName) {
        self.0 = timeline_name.as_str().into();
    }
}
