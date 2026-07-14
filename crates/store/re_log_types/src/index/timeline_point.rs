use re_types_core::TimelineName;

use crate::{TimeInt, TimeType, Timeline};

/// A point on a timeline
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimelinePoint {
    pub name: TimelineName,
    pub typ: TimeType,
    pub time: TimeInt,
}

impl TimelinePoint {
    #[inline]
    pub fn timeline(&self) -> Timeline {
        Timeline::new(self.name, self.typ)
    }

    #[inline]
    pub fn name(&self) -> &TimelineName {
        &self.name
    }

    #[inline]
    pub fn typ(&self) -> TimeType {
        self.typ
    }
}

impl From<(Timeline, TimeInt)> for TimelinePoint {
    #[inline]
    fn from((timeline, time): (Timeline, TimeInt)) -> Self {
        Self {
            name: *timeline.name(),
            typ: timeline.typ(),
            time,
        }
    }
}
