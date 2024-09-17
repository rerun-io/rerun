use super::TimeRangeQuery;
use re_types_core::datatypes::{TimeInt, Utf8};

impl Default for TimeRangeQuery {
    fn default() -> Self {
        Self {
            timeline: Utf8::from("log_time"),
            pov_entity: Default::default(),
            pov_component: Default::default(),
            start: TimeInt::MIN,
            end: TimeInt::MAX,
        }
    }
}
