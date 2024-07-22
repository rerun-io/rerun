use re_types_core::datatypes::{TimeInt, Utf8};

use super::LatestAtQuery;

impl Default for LatestAtQuery {
    fn default() -> Self {
        Self {
            timeline: Utf8::from("log_time"),
            time: TimeInt::MAX,
        }
    }
}
