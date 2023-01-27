use re_log_types::{Time, TimePoint, Timeline};

pub fn log_time() -> TimePoint {
    TimePoint::from([(Timeline::log_time(), Time::now().into())])
}
