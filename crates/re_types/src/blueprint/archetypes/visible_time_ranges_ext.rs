use re_types_core::datatypes::TimeRange;

use super::VisibleTimeRanges;

impl VisibleTimeRanges {
    /// Retrieves the time range for a given timeline.
    pub fn range_for_timeline(&self, timeline_name: &str) -> Option<&'_ TimeRange> {
        self.ranges
            .iter()
            .find(|range| range.timeline.as_str() == timeline_name)
            .map(|range| &range.range)
    }

    /// Sets the time range for a given timeline.
    ///
    /// If the range is `None`, the timeline will be removed from the list of visible time ranges.
    pub fn set_range_for_timeline(&mut self, timeline_name: &str, range: Option<TimeRange>) {
        if let Some(range) = range {
            if let Some(existing_range) = self
                .ranges
                .iter_mut()
                .find(|range| range.timeline.as_str() == timeline_name)
            {
                existing_range.0.range = range;
            } else {
                self.ranges.push(
                    crate::datatypes::VisibleTimeRange {
                        timeline: timeline_name.to_owned().into(),
                        range,
                    }
                    .into(),
                );
            }
        } else {
            self.ranges.retain(|r| r.timeline.as_str() != timeline_name);
        }
    }
}
