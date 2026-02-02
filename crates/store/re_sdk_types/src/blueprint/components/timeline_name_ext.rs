use super::TimelineName;

// Not needed as there is a blanket implementation for impl Into<datatypes::Utf8>
// impl From<re_log_types::TimelineName> for TimelineName {}

impl From<TimelineName> for re_log_types::TimelineName {
    fn from(value: TimelineName) -> Self {
        Self::from(value.as_str())
    }
}

impl From<&TimelineName> for re_log_types::TimelineName {
    fn from(value: &TimelineName) -> Self {
        Self::from(value.as_str())
    }
}

impl TimelineName {
    /// Create a [`Self`] from a [`re_log_types::Timeline`].
    pub fn from_timeline(timeline: &re_log_types::Timeline) -> Self {
        Self::from(timeline.name().as_str())
    }
}

impl Default for TimelineName {
    fn default() -> Self {
        Self::from("log_time")
    }
}
