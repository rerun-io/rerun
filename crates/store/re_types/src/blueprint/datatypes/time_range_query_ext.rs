use super::TimeRangeQuery;
use re_types_core::datatypes::{TimeInt, Utf8};

impl Default for TimeRangeQuery {
    fn default() -> Self {
        Self {
            timeline: Utf8::from("log_time"),
            start: TimeInt::MIN,
            end: TimeInt::MAX,
        }
    }
}
