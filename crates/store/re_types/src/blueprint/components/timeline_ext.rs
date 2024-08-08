use re_log_types::TimelineName;

use super::Timeline;

impl Timeline {
    pub fn timeline_name(&self) -> TimelineName {
        TimelineName::from(self.0.as_str())
    }

    pub fn set_timeline_name(&mut self, timeline_name: TimelineName) {
        self.0 = timeline_name.as_str().into()
    }
}
