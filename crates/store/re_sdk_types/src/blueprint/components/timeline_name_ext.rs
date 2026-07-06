use super::TimelineName;

// Not needed as there is a blanket implementation for impl Into<datatypes::Utf8>
// impl From<re_log_types::TimelineName> for TimelineName {}

// NOTE: fallible, because the blueprint component is a free-form string that can hold
// an empty value, which `re_log_types::TimelineName` forbids.
impl TryFrom<TimelineName> for re_log_types::TimelineName {
    type Error = re_types_core::InvalidTimelineNameError;

    fn try_from(value: TimelineName) -> Result<Self, Self::Error> {
        Self::try_new(value.as_str())
    }
}

impl TryFrom<&TimelineName> for re_log_types::TimelineName {
    type Error = re_types_core::InvalidTimelineNameError;

    fn try_from(value: &TimelineName) -> Result<Self, Self::Error> {
        Self::try_new(value.as_str())
    }
}

impl TimelineName {
    /// Create a [`Self`] from a [`re_log_types::Timeline`].
    pub fn from_timeline(timeline: &re_log_types::Timeline) -> Self {
        Self::from(timeline.name().as_str())
    }

    /// The log time timeline (`"log_time"`).
    pub fn log_time() -> Self {
        Self::from(re_log_types::TimelineName::log_time().as_str())
    }
}

impl Default for TimelineName {
    fn default() -> Self {
        Self::log_time()
    }
}
