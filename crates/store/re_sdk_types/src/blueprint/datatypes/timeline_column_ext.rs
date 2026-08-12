use super::TimelineColumn;

impl Default for TimelineColumn {
    #[inline]
    fn default() -> Self {
        Self {
            timeline: crate::blueprint::components::TimelineName::log_time().0,
            visible: true.into(),
        }
    }
}
