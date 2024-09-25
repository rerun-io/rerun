use re_types_core::datatypes::TimeInt;

use super::range_filter::RangeFilter;

impl Default for RangeFilter {
    fn default() -> Self {
        Self {
            start: TimeInt::MIN,
            end: TimeInt::MAX,
        }
    }
}
